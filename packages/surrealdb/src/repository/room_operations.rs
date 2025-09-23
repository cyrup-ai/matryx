use crate::repository::error::RepositoryError;
use crate::repository::event::EventRepository;
use crate::repository::membership::MembershipRepository;
use crate::repository::relations::{RelationsRepository, RelationsResponse};
use crate::repository::room::{
    ContextResponse,
    MembersResponse,
    RoomRepository,
    RoomUpgradeResponse,
};
use crate::repository::threads::{ThreadInclude, ThreadRootsResponse, ThreadsRepository};
use matryx_entity::types::{MembershipState, SpaceHierarchyResponse as HierarchyResponse};
use serde::{Deserialize, Serialize};
use surrealdb::Connection;

/// Response for room aliases query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasesResponse {
    pub aliases: Vec<String>,
}

/// Membership operation types for validation
#[derive(Debug, Clone)]
pub enum MembershipOperation {
    Invite,
    Ban,
    Kick,
    Leave,
    Join,
    Knock,
}

/// Room member information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMember {
    pub user_id: String,
    pub membership: MembershipState,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub reason: Option<String>,
    pub invited_by: Option<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Coordinated room operations service that orchestrates between repositories
/// This service provides high-level room operations by coordinating between
/// RoomRepository, EventRepository, MembershipRepository, RelationsRepository, and ThreadsRepository
pub struct RoomOperationsService<C: Connection> {
    room_repo: RoomRepository,
    event_repo: EventRepository,
    membership_repo: MembershipRepository,
    relations_repo: RelationsRepository<C>,
    threads_repo: ThreadsRepository<C>,
}

impl<C: Connection> RoomOperationsService<C> {
    pub fn new(
        room_repo: RoomRepository,
        event_repo: EventRepository,
        membership_repo: MembershipRepository,
        relations_repo: RelationsRepository<C>,
        threads_repo: ThreadsRepository<C>,
    ) -> Self {
        Self {
            room_repo,
            event_repo,
            membership_repo,
            relations_repo,
            threads_repo,
        }
    }

    /// Get event context with user permission validation
    pub async fn get_event_context(
        &self,
        room_id: &str,
        event_id: &str,
        limit: u32,
        user_id: &str,
    ) -> Result<ContextResponse, RepositoryError> {
        // Validate user has access to the room
        if !self.membership_repo.is_user_in_room(room_id, user_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} cannot view event context in room {}", user_id, room_id),
            });
        }

        // Get the event context from the room repository
        self.room_repo.get_room_context(room_id, event_id, limit, None).await
    }

    /// Invite a user to a room with validation and event creation
    pub async fn invite_user(
        &self,
        room_id: &str,
        user_id: &str,
        inviter_id: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Validate inviter has permission to invite
        if !self
            .validate_membership_operation(
                room_id,
                inviter_id,
                user_id,
                MembershipOperation::Invite,
            )
            .await?
        {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} cannot invite users to room {}", inviter_id, room_id),
            });
        }

        // Perform the membership operation
        self.membership_repo
            .invite_user_to_room(room_id, user_id, inviter_id, reason.clone())
            .await?;

        // Create the corresponding room event
        let invite_content = serde_json::json!({
            "membership": "invite",
            "displayname": null,
            "avatar_url": null,
            "reason": reason
        });

        self.event_repo
            .create_room_event(
                room_id,
                "m.room.member",
                inviter_id,
                invite_content,
                Some(user_id.to_string()),
            )
            .await?;

        Ok(())
    }

    /// Ban a user from a room with validation and event creation
    pub async fn ban_user(
        &self,
        room_id: &str,
        user_id: &str,
        banner_id: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Validate banner has permission to ban
        if !self
            .validate_membership_operation(room_id, banner_id, user_id, MembershipOperation::Ban)
            .await?
        {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} cannot ban users from room {}", banner_id, room_id),
            });
        }

        // Perform the membership operation
        self.membership_repo
            .ban_user_from_room(room_id, user_id, banner_id, reason.clone())
            .await?;

        // Create the corresponding room event
        let ban_content = serde_json::json!({
            "membership": "ban",
            "reason": reason
        });

        self.event_repo
            .create_room_event(
                room_id,
                "m.room.member",
                banner_id,
                ban_content,
                Some(user_id.to_string()),
            )
            .await?;

        Ok(())
    }

    /// Leave a room with validation and event creation
    pub async fn leave_room(
        &self,
        room_id: &str,
        user_id: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        // User can always leave their own room (no additional validation needed)

        // Perform the membership operation
        self.membership_repo.leave_room(room_id, user_id, reason.clone()).await?;

        // Create the corresponding room event
        let leave_content = serde_json::json!({
            "membership": "leave",
            "reason": reason
        });

        self.event_repo
            .create_room_event(
                room_id,
                "m.room.member",
                user_id,
                leave_content,
                Some(user_id.to_string()),
            )
            .await?;

        Ok(())
    }

    /// Forget a room with validation
    pub async fn forget_room(&self, room_id: &str, user_id: &str) -> Result<(), RepositoryError> {
        // Use both room and membership repositories
        self.room_repo.forget_room(room_id, user_id).await?;
        self.membership_repo.forget_room_membership(room_id, user_id).await?;
        Ok(())
    }

    /// Get room members with user permission validation
    pub async fn get_room_members_with_auth(
        &self,
        room_id: &str,
        user_id: &str,
        at: Option<&str>,
        membership: Option<MembershipState>,
    ) -> Result<MembersResponse, RepositoryError> {
        // Validate user has access to the room
        if !self.membership_repo.is_user_in_room(room_id, user_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} cannot view members of room {}", user_id, room_id),
            });
        }

        // Get members from room repository
        self.room_repo
            .get_room_members_with_filter(room_id, at, membership, None)
            .await
    }

    /// Report an event with validation
    pub async fn report_event(
        &self,
        room_id: &str,
        event_id: &str,
        reporter_id: &str,
        reason: &str,
        score: Option<i32>,
    ) -> Result<(), RepositoryError> {
        // Validate reporter has access to the room
        if !self.membership_repo.is_user_in_room(room_id, reporter_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} cannot report events in room {}", reporter_id, room_id),
            });
        }

        // Report the event
        self.event_repo
            .report_event(room_id, event_id, reporter_id, reason, score)
            .await
    }

    /// Upgrade a room with validation
    pub async fn upgrade_room(
        &self,
        room_id: &str,
        new_version: &str,
        user_id: &str,
    ) -> Result<RoomUpgradeResponse, RepositoryError> {
        // Validate user has permission to upgrade room
        if !self.validate_room_operation(room_id, user_id, "upgrade").await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} cannot upgrade room {}", user_id, room_id),
            });
        }

        // Perform the room upgrade
        self.room_repo.upgrade_room(room_id, new_version, user_id).await
    }

    /// Get room aliases with permission validation
    pub async fn get_room_aliases(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<AliasesResponse, RepositoryError> {
        // Validate user has access to the room
        if !self.membership_repo.is_user_in_room(room_id, user_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} cannot view aliases of room {}", user_id, room_id),
            });
        }

        // Get aliases from room repository
        let aliases = self.room_repo.get_room_aliases(room_id).await?;
        Ok(AliasesResponse { aliases })
    }

    /// Get room hierarchy with permission validation
    pub async fn get_room_hierarchy(
        &self,
        room_id: &str,
        user_id: &str,
        suggested_only: bool,
        max_depth: Option<u32>,
    ) -> Result<HierarchyResponse, RepositoryError> {
        // Validate user has access to the room
        if !self.membership_repo.is_user_in_room(room_id, user_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} cannot view hierarchy of room {}", user_id, room_id),
            });
        }

        // Get hierarchy from room repository
        self.room_repo.get_room_hierarchy(room_id, suggested_only, max_depth).await
    }

    /// Get event relations with permission validation
    pub async fn get_event_relations(
        &self,
        room_id: &str,
        event_id: &str,
        user_id: &str,
        rel_type: Option<&str>,
        event_type: Option<&str>,
    ) -> Result<RelationsResponse, RepositoryError> {
        // Validate user has access to the room
        if !self.membership_repo.is_user_in_room(room_id, user_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} cannot view event relations in room {}", user_id, room_id),
            });
        }

        // Get relations from relations repository
        self.relations_repo
            .get_event_relations(room_id, event_id, rel_type, event_type)
            .await
    }

    /// Get thread roots with permission validation
    pub async fn get_thread_roots(
        &self,
        room_id: &str,
        user_id: &str,
        include: Option<ThreadInclude>,
        since: Option<&str>,
    ) -> Result<ThreadRootsResponse, RepositoryError> {
        // Validate user has access to the room
        if !self.membership_repo.is_user_in_room(room_id, user_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} cannot view thread roots in room {}", user_id, room_id),
            });
        }

        // Get thread roots from threads repository
        self.threads_repo.get_thread_roots(room_id, include, since, None).await
    }

    /// Validate membership operations with power level checks
    pub async fn validate_membership_operation(
        &self,
        room_id: &str,
        actor_id: &str,
        target_id: &str,
        operation: MembershipOperation,
    ) -> Result<bool, RepositoryError> {
        // Check if actor is in the room
        if !self.membership_repo.is_user_in_room(room_id, actor_id).await? {
            return Ok(false);
        }

        // Get actor's power level (simplified - would use PowerLevelsRepository in full implementation)
        let can_perform = match operation {
            MembershipOperation::Invite => {
                // Check if user can invite (simplified validation)
                self.room_repo
                    .validate_room_operation_enhanced(
                        room_id,
                        actor_id,
                        crate::repository::room::RoomOperation::InviteUser,
                    )
                    .await?
            },
            MembershipOperation::Ban => {
                // Check if user can ban (simplified validation)
                self.room_repo
                    .validate_room_operation_enhanced(
                        room_id,
                        actor_id,
                        crate::repository::room::RoomOperation::BanUser,
                    )
                    .await?
            },
            MembershipOperation::Kick => {
                // Check if user can kick (simplified validation)
                self.room_repo
                    .validate_room_operation_enhanced(
                        room_id,
                        actor_id,
                        crate::repository::room::RoomOperation::KickUser,
                    )
                    .await?
            },
            MembershipOperation::Leave => {
                // Users can always leave (unless they're not in the room)
                actor_id == target_id
            },
            MembershipOperation::Join => {
                // Check room join rules (simplified)
                true // Would check actual join rules
            },
            MembershipOperation::Knock => {
                // Check if room allows knocking (simplified)
                true // Would check actual room settings
            },
        };

        Ok(can_perform)
    }

    /// Validate room operations with permission checks
    pub async fn validate_room_operation(
        &self,
        room_id: &str,
        user_id: &str,
        operation: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if user is in the room
        if !self.membership_repo.is_user_in_room(room_id, user_id).await? {
            return Ok(false);
        }

        // Map operation to room operation enum
        let room_operation = match operation {
            "upgrade" => crate::repository::room::RoomOperation::ChangeSettings,
            "send_message" => crate::repository::room::RoomOperation::SendMessage,
            "change_settings" => crate::repository::room::RoomOperation::ChangeSettings,
            "change_power_levels" => crate::repository::room::RoomOperation::ChangePowerLevels,
            _ => return Ok(false),
        };

        // Validate using room repository
        self.room_repo
            .validate_room_operation_enhanced(room_id, user_id, room_operation)
            .await
    }

    /// Get room member list with filtering
    pub async fn get_room_member_list(
        &self,
        room_id: &str,
        at: Option<&str>,
        membership_filter: Option<MembershipState>,
        not_membership_filter: Option<MembershipState>,
    ) -> Result<Vec<RoomMember>, RepositoryError> {
        // Get memberships from membership repository at specific point in time if requested
        let mut memberships = if let Some(state) = membership_filter {
            let user_ids = if let Some(at_event_id) = at {
                // Get membership state at specific event
                self.membership_repo.get_users_by_membership_state_at_event(room_id, state, at_event_id).await?
            } else {
                // Get current membership state
                self.membership_repo.get_users_by_membership_state(room_id, state).await?
            };
            let mut members = Vec::new();
            for user_id in user_ids {
                if let Some(membership) =
                    self.membership_repo.get_membership(room_id, &user_id).await?
                {
                    let member = RoomMember {
                        user_id: membership.user_id,
                        membership: membership.membership,
                        display_name: membership.display_name,
                        avatar_url: membership.avatar_url,
                        reason: membership.reason,
                        invited_by: membership.invited_by,
                        updated_at: membership.updated_at.unwrap_or_else(chrono::Utc::now),
                    };
                    members.push(member);
                }
            }
            members
        } else {
            // Get all members at specific point in time if requested
            let memberships = if let Some(at_event_id) = at {
                self.membership_repo.get_room_members_at_event(room_id, at_event_id).await?
            } else {
                self.membership_repo.get_room_members(room_id).await?
            };
            memberships
                .into_iter()
                .map(|membership| {
                    RoomMember {
                        user_id: membership.user_id,
                        membership: membership.membership,
                        display_name: membership.display_name,
                        avatar_url: membership.avatar_url,
                        reason: membership.reason,
                        invited_by: membership.invited_by,
                        updated_at: membership.updated_at.unwrap_or_else(chrono::Utc::now),
                    }
                })
                .collect()
        };

        // Apply not_membership filtering if specified
        if let Some(not_state) = not_membership_filter {
            memberships.retain(|member| member.membership != not_state);
        }

        Ok(memberships)
    }

    /// Create a new room with proper initialization
    pub async fn create_room(
        &self,
        creator_id: &str,
        config: &crate::repository::room::RoomCreationConfig,
    ) -> Result<String, RepositoryError> {
        // Create the room
        let room = self.room_repo.create_room(config).await?;
        let room_id = room.room_id.clone();

        // Add creator as joined member
        self.membership_repo.join_room(&room_id, creator_id, None, None).await?;

        // Create room creation event
        let creation_content = serde_json::json!({
            "creator": creator_id,
            "room_version": "9",
            "m.federate": true
        });

        self.event_repo
            .create_room_event(
                &room_id,
                "m.room.create",
                creator_id,
                creation_content,
                Some("".to_string()),
            )
            .await?;

        Ok(room_id)
    }

    /// Get comprehensive room information
    pub async fn get_room_info(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<serde_json::Value, RepositoryError> {
        // Validate user has access
        if !self.membership_repo.is_user_in_room(room_id, user_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!("User {} cannot view room info for {}", user_id, room_id),
            });
        }

        // Get room details
        let room = self.room_repo.get_by_id(room_id).await?.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "Room".to_string(),
                id: room_id.to_string(),
            }
        })?;

        // Get membership stats
        let membership_stats = self.membership_repo.get_room_membership_stats(room_id).await?;

        // Get aliases
        let aliases = self.room_repo.get_room_aliases(room_id).await?;

        // Compile comprehensive room info
        let room_info = serde_json::json!({
            "room_id": room.room_id,
            "name": room.name,
            "topic": room.topic,
            "avatar_url": room.avatar_url,
            "canonical_alias": room.canonical_alias,
            "alt_aliases": room.alt_aliases,
            "is_public": room.is_public,
            "is_direct": room.is_direct,
            "join_rules": room.join_rules,
            "guest_access": room.guest_access,
            "history_visibility": room.history_visibility,
            "room_version": room.room_version,
            "created_at": room.created_at,
            "membership_stats": membership_stats,
            "aliases": aliases
        });

        Ok(room_info)
    }

    /// Join a room with comprehensive validation
    pub async fn join_room_with_validation(
        &self,
        room_id: &str,
        user_id: &str,
        display_name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Check room join rules
        let join_rules = self.room_repo.get_room_join_rules(room_id).await?;

        // Validate join permission based on join rules
        let can_join = match join_rules {
            crate::repository::room::JoinRules::Public => true,
            crate::repository::room::JoinRules::Invite => {
                // Check if user has pending invitation
                if let Some(membership) =
                    self.membership_repo.get_membership(room_id, user_id).await?
                {
                    membership.membership == MembershipState::Invite
                } else {
                    false
                }
            },
            crate::repository::room::JoinRules::Knock => {
                // Check if user has knocked and been accepted
                if let Some(membership) =
                    self.membership_repo.get_membership(room_id, user_id).await?
                {
                    membership.membership == MembershipState::Invite
                } else {
                    false
                }
            },
            crate::repository::room::JoinRules::Restricted => {
                // Check restricted join conditions (simplified)
                false // Would implement full restricted join logic
            },
        };

        if !can_join {
            return Err(RepositoryError::Validation {
                field: "join_rules".to_string(),
                message: "User cannot join this room based on current join rules".to_string(),
            });
        }

        // Perform the join
        self.membership_repo
            .join_room(room_id, user_id, display_name.clone(), avatar_url.clone())
            .await?;

        // Create join event
        let join_content = serde_json::json!({
            "membership": "join",
            "displayname": display_name,
            "avatar_url": avatar_url
        });

        self.event_repo
            .create_room_event(
                room_id,
                "m.room.member",
                user_id,
                join_content,
                Some(user_id.to_string()),
            )
            .await?;

        Ok(())
    }
}
