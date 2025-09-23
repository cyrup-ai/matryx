use crate::repository::error::RepositoryError;
use crate::repository::power_levels::PowerLevelAction;
use crate::repository::room::RoomCreationConfig;
use crate::repository::{
    EventRepository,
    MembershipRepository,
    RoomRepository,
    power_levels::PowerLevelsRepository,
};
use matryx_entity::types::{Event, MembershipState, Room};
use serde_json::Value;
use surrealdb::Connection;

pub struct RoomManagementService<C: Connection> {
    room_repo: RoomRepository,
    event_repo: EventRepository,
    membership_repo: MembershipRepository,
    power_levels_repo: PowerLevelsRepository<C>,
}

impl<C: Connection> RoomManagementService<C> {
    pub fn new(
        room_repo: RoomRepository,
        event_repo: EventRepository,
        membership_repo: MembershipRepository,
        power_levels_repo: PowerLevelsRepository<C>,
    ) -> Self {
        Self {
            room_repo,
            event_repo,
            membership_repo,
            power_levels_repo,
        }
    }

    /// Create a new room
    pub async fn create_room(
        &self,
        creator: &str,
        config: RoomCreationConfig,
    ) -> Result<Room, RepositoryError> {
        // Create the room
        let mut room = self.room_repo.create_room(&config).await?;
        room.creator = creator.to_string();

        // Create room creation event
        let creation_content = serde_json::json!({
            "creator": creator,
            "room_version": "9",
            "m.federate": true
        });

        self.event_repo
            .create_room_event(
                &room.room_id,
                "m.room.create",
                creator,
                creation_content,
                Some("".to_string()),
            )
            .await?;

        // Create power levels event with creator as admin
        let power_levels_content = serde_json::json!({
            "users": {
                creator: 100
            },
            "users_default": 0,
            "events": {},
            "events_default": 0,
            "state_default": 50,
            "ban": 50,
            "kick": 50,
            "redact": 50,
            "invite": 50
        });

        self.event_repo
            .create_room_event(
                &room.room_id,
                "m.room.power_levels",
                creator,
                power_levels_content,
                Some("".to_string()),
            )
            .await?;

        // Create join rules event
        let join_rules_content = serde_json::json!({
            "join_rule": if config.is_public { "public" } else { "invite" }
        });

        self.event_repo
            .create_room_event(
                &room.room_id,
                "m.room.join_rules",
                creator,
                join_rules_content,
                Some("".to_string()),
            )
            .await?;

        // Add creator as member
        self.membership_repo
            .create_membership(&matryx_entity::types::Membership {
                user_id: creator.to_string(),
                room_id: room.room_id.clone(),
                membership: MembershipState::Join,
                reason: None,
                invited_by: None,
                updated_at: Some(chrono::Utc::now()),
                display_name: None,
                avatar_url: None,
                is_direct: Some(config.is_direct),
                third_party_invite: None,
                join_authorised_via_users_server: None,
            })
            .await?;

        // Create membership event for creator
        self.event_repo
            .create_membership_change_event(
                &room.room_id,
                creator,
                creator,
                MembershipState::Join,
                None,
            )
            .await?;

        // Set room name if provided
        if let Some(name) = &config.name {
            let name_content = serde_json::json!({
                "name": name
            });
            self.event_repo
                .create_room_event(
                    &room.room_id,
                    "m.room.name",
                    creator,
                    name_content,
                    Some("".to_string()),
                )
                .await?;
        }

        // Set room topic if provided
        if let Some(topic) = &config.topic {
            let topic_content = serde_json::json!({
                "topic": topic
            });
            self.event_repo
                .create_room_event(
                    &room.room_id,
                    "m.room.topic",
                    creator,
                    topic_content,
                    Some("".to_string()),
                )
                .await?;
        }

        // Invite initial users if provided
        for user_id in &config.invite_users {
            self.invite_user_to_room(&room.room_id, creator, user_id).await?;
        }

        Ok(room)
    }

    /// Send an event to a room
    pub async fn send_event(
        &self,
        room_id: &str,
        sender: &str,
        event_type: &str,
        content: Value,
        txn_id: Option<String>,
    ) -> Result<Event, RepositoryError> {
        // Validate that user can send this type of event
        let can_send = if event_type == "m.room.message" {
            self.power_levels_repo
                .can_user_perform_action(room_id, sender, PowerLevelAction::SendMessage)
                .await?
        } else if event_type.starts_with("m.room.") {
            self.power_levels_repo
                .can_user_perform_action(
                    room_id,
                    sender,
                    PowerLevelAction::SendState(event_type.to_string()),
                )
                .await?
        } else {
            self.power_levels_repo
                .can_user_perform_action(room_id, sender, PowerLevelAction::SendMessage)
                .await?
        };

        if !can_send {
            return Err(RepositoryError::Unauthorized {
                reason: "Insufficient power level to send this event type".to_string(),
            });
        }

        // Send the event
        if event_type == "m.room.message" {
            self.event_repo.send_message_event(room_id, sender, content, txn_id).await
        } else {
            // For state events, determine if it needs a state_key
            let state_key = if event_type.starts_with("m.room.") {
                Some("".to_string())
            } else {
                None
            };
            self.event_repo
                .create_room_event(room_id, event_type, sender, content, state_key)
                .await
        }
    }

    /// Kick a user from a room
    pub async fn kick_user(
        &self,
        room_id: &str,
        kicker: &str,
        target: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Check if kicker has permission to kick
        if !self
            .power_levels_repo
            .can_user_perform_action(room_id, kicker, PowerLevelAction::Kick)
            .await?
        {
            return Err(RepositoryError::Unauthorized {
                reason: "Insufficient power level to kick users".to_string(),
            });
        }

        // Validate the membership change
        if !self
            .membership_repo
            .validate_membership_change(room_id, kicker, target, MembershipState::Leave)
            .await?
        {
            return Err(RepositoryError::Validation {
                field: "membership".to_string(),
                message: "Invalid membership change".to_string(),
            });
        }

        // Update membership
        self.membership_repo
            .kick_user(room_id, target, kicker, reason.clone())
            .await?;

        // Create membership event
        self.event_repo
            .create_membership_change_event(room_id, kicker, target, MembershipState::Leave, reason)
            .await?;

        Ok(())
    }

    /// Ban a user from a room
    pub async fn ban_user(
        &self,
        room_id: &str,
        banner: &str,
        target: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Check if banner has permission to ban
        if !self
            .power_levels_repo
            .can_user_perform_action(room_id, banner, PowerLevelAction::Ban)
            .await?
        {
            return Err(RepositoryError::Unauthorized {
                reason: "Insufficient power level to ban users".to_string(),
            });
        }

        // Validate the membership change
        if !self
            .membership_repo
            .validate_membership_change(room_id, banner, target, MembershipState::Ban)
            .await?
        {
            return Err(RepositoryError::Validation {
                field: "membership".to_string(),
                message: "Invalid membership change".to_string(),
            });
        }

        // Update membership
        self.membership_repo
            .ban_user(room_id, target, banner, reason.clone())
            .await?;

        // Create membership event
        self.event_repo
            .create_membership_change_event(room_id, banner, target, MembershipState::Ban, reason)
            .await?;

        Ok(())
    }

    /// Unban a user from a room
    pub async fn unban_user(
        &self,
        room_id: &str,
        unbanner: &str,
        target: &str,
    ) -> Result<(), RepositoryError> {
        // Check if unbanner has permission to ban (same permission needed to unban)
        if !self
            .power_levels_repo
            .can_user_perform_action(room_id, unbanner, PowerLevelAction::Ban)
            .await?
        {
            return Err(RepositoryError::Unauthorized {
                reason: "Insufficient power level to unban users".to_string(),
            });
        }

        // Update membership
        self.membership_repo.unban_user(room_id, target, unbanner).await?;

        // Create membership event
        self.event_repo
            .create_membership_change_event(
                room_id,
                unbanner,
                target,
                MembershipState::Leave,
                Some("Unbanned".to_string()),
            )
            .await?;

        Ok(())
    }

    /// Knock on a room (request to join)
    pub async fn knock_on_room(
        &self,
        room_id_or_alias: &str,
        user_id: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Resolve room ID from alias if necessary
        let actual_room_id = if room_id_or_alias.starts_with('#') {
            match self.room_repo.resolve_room_alias(room_id_or_alias).await? {
                Some(room_id) => room_id,
                None => {
                    return Err(RepositoryError::NotFound {
                        entity_type: "Room alias".to_string(),
                        id: room_id_or_alias.to_string(),
                    });
                },
            }
        } else {
            room_id_or_alias.to_string()
        };

        // Check if room allows knocking
        let join_rules = self.room_repo.get_room_join_rules(&actual_room_id).await?;
        if !matches!(join_rules, crate::repository::room::JoinRules::Knock) {
            return Err(RepositoryError::Validation {
                field: "join_rules".to_string(),
                message: "Room does not allow knocking".to_string(),
            });
        }

        // Create knock membership
        self.membership_repo
            .knock_on_room(&actual_room_id, user_id, reason.clone())
            .await?;

        // Create membership event
        self.event_repo
            .create_membership_change_event(
                &actual_room_id,
                user_id,
                user_id,
                MembershipState::Knock,
                reason,
            )
            .await?;

        Ok(())
    }

    /// Helper method to invite a user to a room
    async fn invite_user_to_room(
        &self,
        room_id: &str,
        inviter: &str,
        user_id: &str,
    ) -> Result<(), RepositoryError> {
        // Check if inviter has permission to invite
        if !self
            .power_levels_repo
            .can_user_perform_action(room_id, inviter, PowerLevelAction::Invite)
            .await?
        {
            return Err(RepositoryError::Unauthorized {
                reason: "Insufficient power level to invite users".to_string(),
            });
        }

        // Create invitation membership
        self.membership_repo.invite_user(room_id, user_id, inviter).await?;

        // Create membership event
        self.event_repo
            .create_membership_change_event(
                room_id,
                inviter,
                user_id,
                MembershipState::Invite,
                None,
            )
            .await?;

        Ok(())
    }
}
