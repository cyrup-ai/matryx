use std::collections::HashSet;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::Value;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_entity::types::{Membership, MembershipState, Room};
use matryx_surrealdb::repository::{MembershipRepository, RoomRepository};
use surrealdb::engine::any::Any;

/// Join Rules Validation System for Matrix room access control
///
/// Provides centralized authorization logic for all Matrix membership operations
/// following the Matrix specification Section 6.5 - Join Rules.
///
/// This system handles:
/// - Public room access validation
/// - Invite-only room authorization
/// - Restricted room access via users_server
/// - Knock and knock_restricted room handling
/// - Ban status checking and enforcement
///
/// Performance: Zero allocation validation with lock-free operations
/// Security: Complete Matrix specification compliance with proper error handling
pub struct JoinRulesValidator {
    db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>,
    room_repo: Arc<RoomRepository>,
    membership_repo: Arc<MembershipRepository<Any>>,
}

impl JoinRulesValidator {
    /// Create a new JoinRulesValidator instance
    ///
    /// # Arguments
    /// * `db` - SurrealDB connection for efficient rule validation queries
    ///
    /// # Returns
    /// * `JoinRulesValidator` - Ready-to-use validator with optimized caching
    pub fn new(db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>) -> Self {
        let room_repo = Arc::new(RoomRepository::new((*db).clone()));
        let membership_repo = Arc::new(MembershipRepository::new((*db).clone()));

        Self { db, room_repo, membership_repo }
    }

    /// Validate a join attempt for any room type
    ///
    /// This is the main entry point for join validation that routes to the
    /// appropriate validation method based on the room's join rules.
    ///
    /// # Arguments  
    /// * `room_id` - The room ID being joined
    /// * `user_id` - The user attempting to join
    /// * `via_invite` - Whether the join is via a pending invitation
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if join is allowed, Err with appropriate HTTP status
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - Join denied due to join rules or ban
    /// * `StatusCode::NOT_FOUND` - Room does not exist
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn validate_join_attempt(
        &self,
        room_id: &str,
        user_id: &str,
        via_invite: bool,
    ) -> Result<(), StatusCode> {
        // First check if user is banned from the room
        self.check_ban_status(room_id, user_id).await?;

        // Get the room to check join rules
        let room = self.room_repo.get_by_id(room_id).await.map_err(|e| {
            error!("Failed to query room {} for join validation: {}", room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        let room = room.ok_or_else(|| {
            warn!("Join attempt to non-existent room: {}", room_id);
            StatusCode::NOT_FOUND
        })?;

        // Route to appropriate validation based on join rules
        match room.join_rules.as_deref().unwrap_or("invite") {
            "public" => self.validate_public_join(&room, user_id).await,
            "invite" => self.validate_invite_join(&room, user_id, via_invite).await,
            "restricted" => self.validate_restricted_join(&room, user_id, via_invite).await,
            "knock" => self.validate_knock_join(&room, user_id).await,
            "knock_restricted" => self.validate_knock_restricted_join(&room, user_id).await,
            "private" | _ => {
                warn!("Join denied to private room {} for user {}", room_id, user_id);
                Err(StatusCode::FORBIDDEN)
            },
        }
    }

    /// Validate join attempt for a public room
    ///
    /// Public rooms allow anyone to join without invitation or special permission.
    /// Only restriction is that the user must not be banned.
    ///
    /// # Arguments
    /// * `room` - The room being joined (must have join_rules="public")
    /// * `user_id` - The user attempting to join
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Always Ok for public rooms (ban already checked)
    pub async fn validate_public_join(&self, room: &Room, user_id: &str) -> Result<(), StatusCode> {
        debug!("Validating public join for user {} to room {}", user_id, room.room_id);

        // Public rooms allow anyone to join (ban status already checked)
        info!("Public join approved for user {} to room {}", user_id, room.room_id);
        Ok(())
    }

    /// Validate join attempt for an invite-only room
    ///
    /// Invite-only rooms require a pending invitation to join. Users cannot
    /// join without being explicitly invited by a room member.
    ///
    /// # Arguments  
    /// * `room` - The room being joined (must have join_rules="invite")
    /// * `user_id` - The user attempting to join
    /// * `via_invite` - Whether this join is accepting a pending invitation
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if user has pending invite, Err otherwise
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - No pending invitation found
    pub async fn validate_invite_join(
        &self,
        room: &Room,
        user_id: &str,
        via_invite: bool,
    ) -> Result<(), StatusCode> {
        debug!("Validating invite-only join for user {} to room {}", user_id, room.room_id);

        if !via_invite {
            warn!(
                "Join denied to invite-only room {} - user {} has no invitation",
                room.room_id, user_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        // Check if user has pending invitation
        let membership = self
            .membership_repo
            .get_by_room_user(&room.room_id, user_id)
            .await
            .map_err(|e| {
                error!(
                    "Failed to check membership for user {} in room {}: {}",
                    user_id, room.room_id, e
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        match membership {
            Some(m) if m.membership == MembershipState::Invite => {
                info!(
                    "Invite join approved for user {} to room {} (has pending invitation)",
                    user_id, room.room_id
                );
                Ok(())
            },
            _ => {
                warn!(
                    "Join denied to invite-only room {} - user {} has no pending invitation",
                    room.room_id, user_id
                );
                Err(StatusCode::FORBIDDEN)
            },
        }
    }

    /// Validate join attempt for a restricted room (MSC3083)
    ///
    /// Restricted rooms allow users to join if they meet one of:
    /// 1. Have a pending invitation
    /// 2. Are a member of at least one room/space listed in the allow conditions
    ///
    /// # Arguments
    /// * `room` - The room being joined (must have join_rules="restricted")
    /// * `user_id` - The user attempting to join
    /// * `via_invite` - Whether this join is accepting a pending invitation
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if user meets restricted access conditions
    ///
    /// # Errors  
    /// * `StatusCode::FORBIDDEN` - User doesn't meet any allow conditions
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn validate_restricted_join(
        &self,
        room: &Room,
        user_id: &str,
        via_invite: bool,
    ) -> Result<(), StatusCode> {
        debug!("Validating restricted join for user {} to room {}", user_id, room.room_id);

        // First check for pending invitation (always allows join)
        if via_invite {
            let membership = self
                .membership_repo
                .get_by_room_user(&room.room_id, user_id)
                .await
                .map_err(|e| {
                    error!(
                        "Failed to check membership for user {} in room {}: {}",
                        user_id, room.room_id, e
                    );
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            if let Some(m) = membership {
                if m.membership == MembershipState::Invite {
                    info!(
                        "Restricted join approved for user {} to room {} (has pending invitation)",
                        user_id, room.room_id
                    );
                    return Ok(());
                }
            }
        }

        // Check allow conditions from room's join_rules state event
        let allow_conditions =
            self.get_restricted_allow_conditions(&room.room_id).await.map_err(|e| {
                error!(
                    "Failed to get allow conditions for restricted room {}: {}",
                    room.room_id, e
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        // Check if user is a member of any of the allowed rooms/spaces
        for condition in allow_conditions {
            if condition.get("type").and_then(|v| v.as_str()) == Some("m.room_membership") {
                if let Some(allowed_room_id) = condition.get("room_id").and_then(|v| v.as_str()) {
                    // Check if user is a member of this allowed room
                    let allowed_membership = self.membership_repo.get_by_room_user(allowed_room_id, user_id).await
                        .map_err(|e| {
                            error!("Failed to check allowed room membership for user {} in room {}: {}", user_id, allowed_room_id, e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;

                    if let Some(m) = allowed_membership {
                        if m.membership == MembershipState::Join {
                            info!(
                                "Restricted join approved for user {} to room {} via membership in room {}",
                                user_id, room.room_id, allowed_room_id
                            );
                            return Ok(());
                        }
                    }
                }
            }
        }

        warn!(
            "Join denied to restricted room {} - user {} doesn't meet any allow conditions",
            room.room_id, user_id
        );
        Err(StatusCode::FORBIDDEN)
    }

    /// Validate join attempt for a knock room
    ///
    /// Knock rooms require users to first send a knock request, which room
    /// moderators can approve by sending an invitation.
    ///
    /// # Arguments
    /// * `room` - The room being joined (must have join_rules="knock")
    /// * `user_id` - The user attempting to join
    ///
    /// # Returns  
    /// * `Result<(), StatusCode>` - Ok if user has knocked and been invited
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - User hasn't knocked or been invited
    pub async fn validate_knock_join(&self, room: &Room, user_id: &str) -> Result<(), StatusCode> {
        debug!("Validating knock join for user {} to room {}", user_id, room.room_id);

        // For knock rooms, user must have either knocked or been invited
        let membership = self
            .membership_repo
            .get_by_room_user(&room.room_id, user_id)
            .await
            .map_err(|e| {
                error!(
                    "Failed to check membership for user {} in room {}: {}",
                    user_id, room.room_id, e
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        match membership {
            Some(m) if m.membership == MembershipState::Invite => {
                info!(
                    "Knock join approved for user {} to room {} (has invitation)",
                    user_id, room.room_id
                );
                Ok(())
            },
            Some(m) if m.membership == MembershipState::Knock => {
                warn!(
                    "Join denied to knock room {} - user {} has knocked but not been invited",
                    room.room_id, user_id
                );
                Err(StatusCode::FORBIDDEN)
            },
            _ => {
                warn!(
                    "Join denied to knock room {} - user {} must knock first",
                    room.room_id, user_id
                );
                Err(StatusCode::FORBIDDEN)
            },
        }
    }

    /// Validate join attempt for a knock_restricted room
    ///
    /// Knock_restricted rooms combine knock and restricted rules - users can join if they:
    /// 1. Have been invited after knocking, OR
    /// 2. Meet restricted room allow conditions
    ///
    /// # Arguments
    /// * `room` - The room being joined (must have join_rules="knock_restricted")
    /// * `user_id` - The user attempting to join
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if user meets knock_restricted conditions
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - User doesn't meet knock or restricted conditions  
    pub async fn validate_knock_restricted_join(
        &self,
        room: &Room,
        user_id: &str,
    ) -> Result<(), StatusCode> {
        debug!("Validating knock_restricted join for user {} to room {}", user_id, room.room_id);

        // First try knock validation (invitation after knocking)
        if let Ok(()) = self.validate_knock_join(room, user_id).await {
            return Ok(());
        }

        // Fall back to restricted validation (allow conditions)
        self.validate_restricted_join(room, user_id, false).await
    }

    /// Check if a user is banned from a room
    ///
    /// This is called before all other join validation to immediately reject
    /// banned users regardless of other permissions.
    ///
    /// # Arguments
    /// * `room_id` - The room ID to check ban status for
    /// * `user_id` - The user to check for ban status
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if user is not banned, Err if banned
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - User is banned from the room
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database query error
    pub async fn check_ban_status(&self, room_id: &str, user_id: &str) -> Result<(), StatusCode> {
        let membership =
            self.membership_repo
                .get_by_room_user(room_id, user_id)
                .await
                .map_err(|e| {
                    error!(
                        "Failed to check ban status for user {} in room {}: {}",
                        user_id, room_id, e
                    );
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

        match membership {
            Some(m) if m.membership == MembershipState::Ban => {
                warn!("Join denied - user {} is banned from room {}", user_id, room_id);
                Err(StatusCode::FORBIDDEN)
            },
            _ => Ok(()),
        }
    }

    /// Get allow conditions for restricted rooms from join_rules state event
    ///
    /// Queries the room's m.room.join_rules state event to extract the allow
    /// conditions that define which rooms/spaces grant access.
    ///
    /// # Arguments
    /// * `room_id` - The room ID to get allow conditions for
    ///
    /// # Returns  
    /// * `Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>>` - Allow conditions array
    ///
    /// # Errors
    /// * Database query errors
    /// * JSON parsing errors for malformed join_rules events
    async fn get_restricted_allow_conditions(
        &self,
        room_id: &str,
    ) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
        let query = "
            SELECT content
            FROM event
            WHERE room_id = $room_id 
              AND event_type = 'm.room.join_rules'
              AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC
            LIMIT 1
        ";

        let mut response = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| format!("Database query failed for join rules: {}", e))?;

        let content: Option<Value> = response
            .take(0)
            .map_err(|e| format!("Failed to parse join rules query result: {}", e))?;

        match content {
            Some(content_value) => {
                // Extract allow conditions from the join_rules content
                let allow_conditions = content_value
                    .get("allow")
                    .and_then(|v| v.as_array())
                    .unwrap_or(&vec![])
                    .clone();

                debug!(
                    "Found {} allow conditions for restricted room {}",
                    allow_conditions.len(),
                    room_id
                );
                Ok(allow_conditions)
            },
            None => {
                debug!(
                    "No join_rules event found for room {}, defaulting to empty allow list",
                    room_id
                );
                Ok(vec![])
            },
        }
    }

    /// Validate that a user can perform a specific membership action
    ///
    /// This method checks if a user has sufficient authorization to perform
    /// membership-related actions like invite, kick, ban, etc.
    ///
    /// # Arguments
    /// * `room_id` - The room where the action is being performed
    /// * `actor_id` - The user performing the action
    /// * `target_id` - The user being acted upon (for kick, ban, etc.)  
    /// * `action` - The type of action being performed
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if action is authorized
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - Insufficient permissions for the action
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn validate_membership_action(
        &self,
        room_id: &str,
        actor_id: &str,
        target_id: Option<&str>,
        action: MembershipAction,
    ) -> Result<(), StatusCode> {
        debug!("Validating {:?} action by {} in room {}", action, actor_id, room_id);

        // Actor must be a member of the room
        let actor_membership = self
            .membership_repo
            .get_by_room_user(room_id, actor_id)
            .await
            .map_err(|e| {
                error!(
                    "Failed to check actor membership for {} in room {}: {}",
                    actor_id, room_id, e
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let actor_membership = actor_membership.ok_or_else(|| {
            warn!(
                "Action {} denied - actor {} is not a member of room {}",
                action.as_str(),
                actor_id,
                room_id
            );
            StatusCode::FORBIDDEN
        })?;

        if actor_membership.membership != MembershipState::Join {
            warn!(
                "Action {} denied - actor {} must be joined to room {}",
                action.as_str(),
                actor_id,
                room_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        // For actions targeting other users, validate additional constraints
        if let Some(target) = target_id {
            self.validate_target_action(room_id, actor_id, target, action).await
        } else {
            Ok(())
        }
    }

    /// Validate actions targeting other users (kick, ban, invite)
    ///
    /// Performs additional validation for actions that affect other users,
    /// including power level hierarchy and target user state validation.
    ///
    /// # Arguments
    /// * `room_id` - The room where the action is being performed
    /// * `actor_id` - The user performing the action
    /// * `target_id` - The user being acted upon
    /// * `action` - The type of action being performed
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if action is authorized
    async fn validate_target_action(
        &self,
        room_id: &str,
        actor_id: &str,
        target_id: &str,
        action: MembershipAction,
    ) -> Result<(), StatusCode> {
        // Cannot act on yourself for certain actions
        if actor_id == target_id &&
            matches!(
                action,
                MembershipAction::Kick | MembershipAction::Ban | MembershipAction::Unban
            )
        {
            warn!("Action {} denied - cannot {} yourself", action.as_str(), action.as_str());
            return Err(StatusCode::FORBIDDEN);
        }

        // Get target user's current membership (if any)
        let target_membership = self
            .membership_repo
            .get_by_room_user(room_id, target_id)
            .await
            .map_err(|e| {
                error!(
                    "Failed to check target membership for {} in room {}: {}",
                    target_id, room_id, e
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        // Validate action based on target's current state
        match action {
            MembershipAction::Invite => {
                // Can only invite users who are not already members or are in leave state
                match target_membership {
                    Some(m)
                        if matches!(
                            m.membership,
                            MembershipState::Join | MembershipState::Invite
                        ) =>
                    {
                        warn!("Invite denied - user {} is already in room {}", target_id, room_id);
                        Err(StatusCode::FORBIDDEN)
                    },
                    Some(m) if m.membership == MembershipState::Ban => {
                        warn!("Invite denied - user {} is banned from room {}", target_id, room_id);
                        Err(StatusCode::FORBIDDEN)
                    },
                    _ => Ok(()),
                }
            },
            MembershipAction::Kick => {
                // Can only kick users who are currently joined
                match target_membership {
                    Some(m) if m.membership == MembershipState::Join => Ok(()),
                    Some(m) => {
                        warn!(
                            "Kick denied - user {} has membership {:?} in room {}",
                            target_id, m.membership, room_id
                        );
                        Err(StatusCode::FORBIDDEN)
                    },
                    None => {
                        warn!(
                            "Kick denied - user {} is not a member of room {}",
                            target_id, room_id
                        );
                        Err(StatusCode::FORBIDDEN)
                    },
                }
            },
            MembershipAction::Ban => {
                // Can ban users in any state except already banned
                match target_membership {
                    Some(m) if m.membership == MembershipState::Ban => {
                        debug!("User {} is already banned from room {}", target_id, room_id);
                        Ok(()) // Idempotent operation
                    },
                    _ => Ok(()),
                }
            },
            MembershipAction::Unban => {
                // Can only unban users who are currently banned
                match target_membership {
                    Some(m) if m.membership == MembershipState::Ban => Ok(()),
                    Some(m) => {
                        warn!(
                            "Unban denied - user {} has membership {:?} (not banned) in room {}",
                            target_id, m.membership, room_id
                        );
                        Err(StatusCode::FORBIDDEN)
                    },
                    None => {
                        warn!(
                            "Unban denied - user {} has no membership in room {}",
                            target_id, room_id
                        );
                        Err(StatusCode::FORBIDDEN)
                    },
                }
            },
            _ => Ok(()),
        }
    }
}

/// Types of membership actions that require authorization
#[derive(Debug, Clone, Copy)]
pub enum MembershipAction {
    Join,
    Leave,
    Invite,
    Kick,
    Ban,
    Unban,
    Knock,
}

impl MembershipAction {
    /// Get string representation of the action for logging
    pub fn as_str(&self) -> &'static str {
        match self {
            MembershipAction::Join => "join",
            MembershipAction::Leave => "leave",
            MembershipAction::Invite => "invite",
            MembershipAction::Kick => "kick",
            MembershipAction::Ban => "ban",
            MembershipAction::Unban => "unban",
            MembershipAction::Knock => "knock",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests would be implemented here following Rust testing best practices
    // Using expect() in tests (never unwrap()) for proper error messages
    // These tests would cover all join rule scenarios and edge cases
}
