use crate::repository::error::RepositoryError;
use crate::repository::membership::{MembershipContext, MembershipRepository};
use crate::repository::room::{RoomRepository, JoinRules};
use crate::repository::room_alias::RoomAliasRepository;
use matryx_entity::types::{Event, MembershipState};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use surrealdb::{Surreal, engine::any::Any};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationResult {
    pub authorized: bool,
    pub reason: Option<String>,
    pub required_power_level: Option<i64>,
    pub user_power_level: Option<i64>,
    pub error_code: Option<String>,
    pub details: Option<serde_json::Value>,
}

impl AuthorizationResult {
    pub fn authorized() -> Self {
        Self {
            authorized: true,
            reason: None,
            required_power_level: None,
            user_power_level: None,
            error_code: None,
            details: None,
        }
    }

    pub fn denied(reason: &str) -> Self {
        Self {
            authorized: false,
            reason: Some(reason.to_string()),
            required_power_level: None,
            user_power_level: None,
            error_code: None,
            details: None,
        }
    }

    pub fn insufficient_power_level(required: i64, current: i64) -> Self {
        Self {
            authorized: false,
            reason: Some(format!("Insufficient power level: required {}, current {}", required, current)),
            required_power_level: Some(required),
            user_power_level: Some(current),
            error_code: Some("M_FORBIDDEN".to_string()),
            details: None,
        }
    }
}

pub struct RoomAuthorizationService {
    room_repo: Arc<RoomRepository>,
    membership_repo: Arc<MembershipRepository>,
    room_alias_repo: Arc<RoomAliasRepository>,
    db: Surreal<Any>,
}

impl RoomAuthorizationService {
    pub fn new(
        room_repo: Arc<RoomRepository>,
        membership_repo: Arc<MembershipRepository>,
        room_alias_repo: Arc<RoomAliasRepository>,
        db: Surreal<Any>,
    ) -> Self {
        Self {
            room_repo,
            membership_repo,
            room_alias_repo,
            db,
        }
    }

    /// Get room alias repository for resolving aliases in authorization context
    pub fn room_alias_repo(&self) -> &Arc<RoomAliasRepository> {
        &self.room_alias_repo
    }

    /// Check if user has access to perform an action in a room
    pub async fn check_room_access(
        &self,
        room_id: &str,
        user_id: &str,
        action: &str,
    ) -> Result<AuthorizationResult, RepositoryError> {
        // Check if room exists
        let room = self.room_repo.get_by_id(room_id).await?;
        if room.is_none() {
            return Ok(AuthorizationResult::denied("Room not found"));
        }

        // Check if user is a member of the room
        let membership = self.membership_repo.get_membership(room_id, user_id).await?;
        let is_joined = membership.as_ref().map(|m| m.membership == MembershipState::Join).unwrap_or(false);
        if !is_joined {
            // For some actions, non-members might be allowed
            match action {
                "read_public" | "peek" => {
                    // Check if room is world readable
                    let visibility = self.room_repo.get_room_visibility(room_id).await?;
                    match visibility {
                        crate::repository::room::RoomVisibility::Public => return Ok(AuthorizationResult::authorized()),
                        crate::repository::room::RoomVisibility::Private => {
                            return Ok(AuthorizationResult::denied("Room is not world readable"));
                        }
                    }
                },
                "join" => {
                    // Check join rules
                    return self.validate_join_request(room_id, user_id, None).await;
                },
                _ => {
                    return Ok(AuthorizationResult::denied("Not a member of the room"));
                }
            }
        }

        // Get user's power level
        let user_power_level = self.room_repo.get_user_power_level(room_id, user_id).await?;
        let power_levels = self.room_repo.get_room_power_levels(room_id).await?;

        // Check power level requirements for different actions
        let required_level = match action {
            "send_message" => {
                power_levels.events.get("m.room.message").copied().unwrap_or(power_levels.events_default)
            },
            "send_state" => power_levels.state_default,
            "invite" => power_levels.invite,
            "kick" => power_levels.kick,
            "ban" => power_levels.ban,
            "redact" => power_levels.redact,
            "change_power_levels" => {
                power_levels.events.get("m.room.power_levels").copied().unwrap_or(100)
            },
            "change_name" => {
                power_levels.events.get("m.room.name").copied().unwrap_or(power_levels.state_default)
            },
            "change_topic" => {
                power_levels.events.get("m.room.topic").copied().unwrap_or(power_levels.state_default)
            },
            "change_avatar" => {
                power_levels.events.get("m.room.avatar").copied().unwrap_or(power_levels.state_default)
            },
            "change_canonical_alias" => {
                power_levels.events.get("m.room.canonical_alias").copied().unwrap_or(power_levels.state_default)
            },
            "change_join_rules" => {
                power_levels.events.get("m.room.join_rules").copied().unwrap_or(power_levels.state_default)
            },
            "change_history_visibility" => {
                power_levels.events.get("m.room.history_visibility").copied().unwrap_or(power_levels.state_default)
            },
            "upgrade_room" => {
                power_levels.events.get("m.room.tombstone").copied().unwrap_or(100)
            },
            "read" | "read_timeline" => 0, // Any member can read
            _ => {
                // Unknown action, check if it's a state event
                if action.starts_with("m.room.") {
                    power_levels.state_default
                } else {
                    power_levels.events_default
                }
            }
        };

        if user_power_level >= required_level {
            Ok(AuthorizationResult::authorized())
        } else {
            Ok(AuthorizationResult::insufficient_power_level(required_level, user_power_level))
        }
    }

    /// Validate a join request
    pub async fn validate_join_request(
        &self,
        room_id: &str,
        user_id: &str,
        via_server: Option<&str>,
    ) -> Result<AuthorizationResult, RepositoryError> {
        // Check if room exists
        let room = self.room_repo.get_by_id(room_id).await?;
        if room.is_none() {
            return Ok(AuthorizationResult::denied("Room not found"));
        }

        // Check current membership
        let current_membership = self.membership_repo.get_membership(room_id, user_id).await?;
        if let Some(membership) = current_membership {
            match membership.membership {
                MembershipState::Join => {
                    return Ok(AuthorizationResult::denied("User is already joined"));
                },
                MembershipState::Ban => {
                    return Ok(AuthorizationResult::denied("User is banned from the room"));
                },
                MembershipState::Invite => {
                    // User has invitation, can join
                    return Ok(AuthorizationResult::authorized());
                },
                _ => {
                    // Continue with join rules check
                }
            }
        }

        // Check join rules
        let join_rules = self.room_repo.get_room_join_rules(room_id).await?;
        match join_rules {
            JoinRules::Public => Ok(AuthorizationResult::authorized()),
            JoinRules::Invite => {
                Ok(AuthorizationResult::denied("Room requires invitation to join"))
            },
            JoinRules::Knock => {
                Ok(AuthorizationResult::denied("Room requires knocking before joining"))
            },
            JoinRules::Private => {
                Ok(AuthorizationResult::denied("Room is private and cannot be joined"))
            },
            JoinRules::Restricted => {
                // Check if user meets restricted room requirements
                self.validate_restricted_join(room_id, user_id, via_server).await
            },
        }
    }

    /// Authorize a state change event
    pub async fn authorize_state_change(
        &self,
        room_id: &str,
        event: &Event,
        auth_chain: &[String],
    ) -> Result<AuthorizationResult, RepositoryError> {
        let sender = &event.sender;
        let event_type = &event.event_type;

        // Check if sender is in the room
        let sender_membership = self.membership_repo.get_membership(room_id, sender).await?;
        let sender_is_joined = sender_membership.as_ref().map(|m| m.membership == MembershipState::Join).unwrap_or(false);
        if !sender_is_joined {
            // Exception: create event doesn't require membership
            if event_type != "m.room.create" {
                return Ok(AuthorizationResult::denied("Sender is not a member of the room"));
            }
        }

        // Validate auth chain
        if !self.validate_auth_chain(room_id, auth_chain).await? {
            return Ok(AuthorizationResult::denied("Invalid authorization chain"));
        }

        // Check specific event type authorization
        match event_type.as_str() {
            "m.room.create" => {
                // Room creation is always authorized for the creator
                Ok(AuthorizationResult::authorized())
            },
            "m.room.member" => {
                // Membership events need special handling
                if let Some(_state_key) = &event.state_key {
                    let membership_context = self.extract_membership_context(event)?;
                    self.coordinate_membership_auth(room_id, &membership_context).await
                } else {
                    Ok(AuthorizationResult::denied("Membership event missing state_key"))
                }
            },
            _ => {
                // Regular state events - check power levels
                self.check_room_access(room_id, sender, event_type).await
            }
        }
    }

    /// Check if user meets power level requirement
    pub async fn check_power_level_requirement(
        &self,
        room_id: &str,
        user_id: &str,
        required_level: i64,
    ) -> Result<bool, RepositoryError> {
        let user_power_level = self.room_repo.get_user_power_level(room_id, user_id).await?;
        Ok(user_power_level >= required_level)
    }

    /// Validate a room operation
    pub async fn validate_room_operation(
        &self,
        room_id: &str,
        user_id: &str,
        operation: &str,
        target: Option<&str>,
    ) -> Result<AuthorizationResult, RepositoryError> {
        // Check basic room access first
        let base_result = self.check_room_access(room_id, user_id, operation).await?;
        if !base_result.authorized {
            return Ok(base_result);
        }

        // Additional validation for operations with targets
        if let Some(target_user) = target {
            match operation {
                "kick" | "ban" => {
                    // Can't kick/ban users with equal or higher power level
                    let user_power = self.room_repo.get_user_power_level(room_id, user_id).await?;
                    let target_power = self.room_repo.get_user_power_level(room_id, target_user).await?;
                    
                    if user_power <= target_power {
                        return Ok(AuthorizationResult::denied(
                            "Cannot kick/ban user with equal or higher power level"
                        ));
                    }
                },
                "invite" => {
                    // Check if target is already in room
                    let target_membership = self.membership_repo.get_membership(room_id, target_user).await?;
                    if let Some(membership) = target_membership {
                        match membership.membership {
                            MembershipState::Join => {
                                return Ok(AuthorizationResult::denied("User is already joined"));
                            },
                            MembershipState::Invite => {
                                return Ok(AuthorizationResult::denied("User is already invited"));
                            },
                            MembershipState::Ban => {
                                // Need unban power to invite banned user
                                let ban_level = self.room_repo.get_room_power_levels(room_id).await?.ban;
                                if !self.check_power_level_requirement(room_id, user_id, ban_level).await? {
                                    return Ok(AuthorizationResult::denied("Cannot invite banned user without ban permissions"));
                                }
                            },
                            _ => {
                                // Can invite
                            }
                        }
                    }
                },
                _ => {
                    // Other operations don't need target validation
                }
            }
        }

        Ok(AuthorizationResult::authorized())
    }

    /// Coordinate membership authorization
    pub async fn coordinate_membership_auth(
        &self,
        room_id: &str,
        membership_context: &MembershipContext,
    ) -> Result<AuthorizationResult, RepositoryError> {
        let sender = &membership_context.sender;
        let target_user = &membership_context.user_id;
        let new_membership = membership_context.membership.clone();

        // Self membership changes
        if sender == target_user {
            match new_membership {
                MembershipState::Join => {
                    // Validate join request
                    self.validate_join_request(room_id, target_user, None).await
                },
                MembershipState::Leave => {
                    // Users can always leave
                    Ok(AuthorizationResult::authorized())
                },
                MembershipState::Knock => {
                    // Check if room allows knocking
                    let join_rules = self.room_repo.get_room_join_rules(room_id).await?;
                    match join_rules {
                        JoinRules::Knock | JoinRules::Restricted => Ok(AuthorizationResult::authorized()),
                        _ => Ok(AuthorizationResult::denied("Room does not allow knocking")),
                    }
                },
                MembershipState::Invite | MembershipState::Ban => {
                    Ok(AuthorizationResult::denied("Cannot self-invite or self-ban"))
                },
            }
        } else {
            // Other-user membership changes
            match new_membership {
                MembershipState::Invite => {
                    self.validate_room_operation(room_id, sender, "invite", Some(target_user)).await
                },
                MembershipState::Ban => {
                    self.validate_room_operation(room_id, sender, "ban", Some(target_user)).await
                },
                MembershipState::Leave => {
                    // This could be a kick
                    self.validate_room_operation(room_id, sender, "kick", Some(target_user)).await
                },
                MembershipState::Join => {
                    Ok(AuthorizationResult::denied("Cannot join on behalf of another user"))
                },
                MembershipState::Knock => {
                    Ok(AuthorizationResult::denied("Cannot knock on behalf of another user"))
                },
            }
        }
    }

    // Helper methods

    /// Validate restricted room join requirements
    async fn validate_restricted_join(
        &self,
        room_id: &str,
        user_id: &str,
        _via_server: Option<&str>,
    ) -> Result<AuthorizationResult, RepositoryError> {
        // Get join rule allow conditions
        let allow_conditions = self.room_repo.get_join_rule_allow_conditions(room_id).await?;
        
        if allow_conditions.is_empty() {
            return Ok(AuthorizationResult::denied("Restricted room has no allow conditions"));
        }

        // Check each allow condition
        for condition in allow_conditions {
            if let Some("m.room_membership") = condition.get("type").and_then(|v| v.as_str()) {
                // User must be member of specified room
                if let Some(room_id_condition) = condition.get("room_id").and_then(|v| v.as_str())
                    && self.membership_repo.is_user_in_room(room_id_condition, user_id).await?
                {
                    return Ok(AuthorizationResult::authorized());
                }
            }
        }

        Ok(AuthorizationResult::denied("User does not meet restricted room requirements"))
    }

    /// Validate authorization chain
    async fn validate_auth_chain(
        &self,
        room_id: &str,
        auth_chain: &[String],
    ) -> Result<bool, RepositoryError> {
        // Basic validation: auth chain should contain room creation event
        for auth_event_id in auth_chain {
            let query = "SELECT event_type FROM event WHERE event_id = $event_id AND room_id = $room_id";
            let mut result = self.db
                .query(query)
                .bind(("event_id", auth_event_id.to_string()))
                .bind(("room_id", room_id.to_string()))
                .await?;
            let events: Vec<serde_json::Value> = result.take(0)?;

            if let Some(event) = events.first()
                && let Some(event_type) = event.get("event_type").and_then(|v| v.as_str())
                && event_type == "m.room.create"
            {
                return Ok(true);
            }
        }

        // No create event found in auth chain
        Ok(false)
    }

    /// Extract membership context from event
    fn extract_membership_context(&self, event: &Event) -> Result<MembershipContext, RepositoryError> {
        let target_user = event.state_key.as_ref().ok_or_else(|| {
            RepositoryError::Validation {
                field: "state_key".to_string(),
                message: "Membership event must have state_key".to_string(),
            }
        })?;

        let content_value = serde_json::to_value(&event.content)?;
        let membership_str = content_value
            .get("membership")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RepositoryError::Validation {
                field: "membership".to_string(),
                message: "Membership event must have membership field".to_string(),
            })?;

        let membership = match membership_str {
            "join" => MembershipState::Join,
            "leave" => MembershipState::Leave,
            "invite" => MembershipState::Invite,
            "ban" => MembershipState::Ban,
            "knock" => MembershipState::Knock,
            _ => return Err(RepositoryError::Validation {
                field: "membership".to_string(),
                message: format!("Invalid membership state: {}", membership_str),
            }),
        };

        Ok(MembershipContext {
            user_id: target_user.clone(),
            room_id: event.room_id.clone(),
            membership,
            sender: event.sender.clone(),
            reason: content_value
                .get("reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            invited_by: content_value
                .get("invited_by")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            display_name: content_value
                .get("displayname")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            avatar_url: content_value
                .get("avatar_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            is_direct: content_value.get("is_direct").and_then(|v| v.as_bool()),
            third_party_invite: content_value.get("third_party_invite").cloned(),
            join_authorised_via_users_server: content_value
                .get("join_authorised_via_users_server")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            event_id: event.event_id.clone(),
            origin_server_ts: event.origin_server_ts,
            auth_events: event.auth_events.clone().unwrap_or_default(),
            prev_events: event.prev_events.clone().unwrap_or_default(),
        })
    }

    /// Check if user has administrative privileges in room
    pub async fn is_room_admin(&self, room_id: &str, user_id: &str) -> Result<bool, RepositoryError> {
        let user_power_level = self.room_repo.get_user_power_level(room_id, user_id).await?;
        Ok(user_power_level >= 100)
    }

    /// Check if user has moderator privileges in room
    pub async fn is_room_moderator(&self, room_id: &str, user_id: &str) -> Result<bool, RepositoryError> {
        let user_power_level = self.room_repo.get_user_power_level(room_id, user_id).await?;
        Ok(user_power_level >= 50)
    }

    /// Get effective power level for action
    pub async fn get_effective_power_level(
        &self,
        room_id: &str,
        action: &str,
    ) -> Result<i64, RepositoryError> {
        let power_levels = self.room_repo.get_room_power_levels(room_id).await?;
        
        let effective_level = match action {
            "ban" => power_levels.ban,
            "kick" => power_levels.kick,
            "invite" => power_levels.invite,
            "redact" => power_levels.redact,
            _ => {
                if action.starts_with("m.room.") {
                    power_levels.events.get(action).copied().unwrap_or(power_levels.state_default)
                } else {
                    power_levels.events_default
                }
            }
        };

        Ok(effective_level)
    }
}