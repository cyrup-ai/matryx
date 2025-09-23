use crate::repository::error::RepositoryError;
use crate::repository::{EventRepository, MembershipRepository, RoomRepository, UserRepository};
use matryx_entity::types::{Membership, MembershipState};


#[derive(Debug)]
pub struct JoinResult {
    pub room_id: String,
    pub event_id: String,
    pub success: bool,
}

pub struct RoomJoinService {
    room_repo: RoomRepository,
    membership_repo: MembershipRepository,
    event_repo: EventRepository,
    user_repo: UserRepository,
}

impl RoomJoinService {
    pub fn new(
        room_repo: RoomRepository,
        membership_repo: MembershipRepository,
        event_repo: EventRepository,
        user_repo: UserRepository,
    ) -> Self {
        Self { room_repo, membership_repo, event_repo, user_repo }
    }

    /// Join a room by room ID or alias
    pub async fn join_room(
        &self,
        room_id_or_alias: &str,
        user_id: &str,
    ) -> Result<JoinResult, RepositoryError> {
        // Resolve room ID from alias if necessary
        let actual_room_id = if room_id_or_alias.starts_with('#') {
            // Room alias - need to resolve to room ID
            match self.room_repo.resolve_room_alias(room_id_or_alias).await? {
                Some(room_id) => room_id,
                None => {
                    return Err(RepositoryError::NotFound {
                        entity_type: "Room alias".to_string(),
                        id: room_id_or_alias.to_string(),
                    });
                },
            }
        } else if room_id_or_alias.starts_with('!') {
            // Already a room ID
            room_id_or_alias.to_string()
        } else {
            return Err(RepositoryError::Validation {
                field: "room_id_or_alias".to_string(),
                message: "Invalid room identifier format".to_string(),
            });
        };

        // Validate join request
        if !self.validate_join_request(&actual_room_id, user_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: "User not authorized to join room".to_string(),
            });
        }

        // Process the join
        let event_id = self.process_join(&actual_room_id, user_id).await?;

        Ok(JoinResult { room_id: actual_room_id, event_id, success: true })
    }

    /// Validate if a user can join a room
    pub async fn validate_join_request(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if user is valid for joining
        if !self.user_repo.validate_user_for_join(user_id).await? {
            return Ok(false);
        }

        // Check if room exists
        if self.room_repo.get_room_by_id(room_id).await?.is_none() {
            return Ok(false);
        }

        // Check if user is already in the room
        if let Some(membership) = self.membership_repo.get_membership(room_id, user_id).await? {
            match membership.membership {
                MembershipState::Join => return Ok(false), // Already joined
                MembershipState::Ban => return Ok(false),  // Banned from room
                _ => {},                                   // Continue with other checks
            }
        }

        // Check if room is joinable by this user
        self.room_repo.is_room_joinable(room_id, user_id).await
    }

    /// Process the actual room join operation
    pub async fn process_join(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<String, RepositoryError> {
        // Create membership event
        let join_event = self
            .event_repo
            .create_membership_event(room_id, user_id, MembershipState::Join)
            .await?;

        // Create/update membership record
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Join,
            reason: None,
            invited_by: None,
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        self.membership_repo.create_membership(&membership).await?;

        Ok(join_event.event_id)
    }
}
