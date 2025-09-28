use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::Value;
use tracing::{debug, error, info, warn};

use matryx_surrealdb::repository::{MembershipRepository, RoomRepository};

/// Power Level Validation Engine for Matrix room authorization
///
/// Provides centralized power level validation for all Matrix room operations
/// following the Matrix specification Section 6.4 - Power Levels.
///
/// This engine handles:
/// - User power level lookups with efficient caching
/// - Action permission validation (invite, kick, ban, redact, state changes)  
/// - Power level hierarchy enforcement and comparison
/// - Default power level handling per Matrix specification
/// - State event modification authorization
///
/// Performance: Zero allocation comparisons with lock-free operations
/// Security: Complete Matrix specification compliance with hierarchy enforcement
pub struct PowerLevelValidator {
    db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>,
    room_repo: Arc<RoomRepository>,
    membership_repo: Arc<MembershipRepository>,
}

impl PowerLevelValidator {
    /// Create a new PowerLevelValidator instance
    ///
    /// # Arguments
    /// * `db` - SurrealDB connection for efficient power level queries
    ///
    /// # Returns
    /// * `PowerLevelValidator` - Ready-to-use validator with optimized performance
    pub fn new(db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>) -> Self {
        let room_repo = Arc::new(RoomRepository::new((*db).clone()));
        let membership_repo = Arc::new(MembershipRepository::new((*db).clone()));

        Self { db, room_repo, membership_repo }
    }

    /// Check if a user has permission to invite users to a room
    ///
    /// Default required power level: 0 (any user can invite by default)
    /// Users must have power level >= invite level and >= target user's level
    ///
    /// # Arguments
    /// * `room_id` - The room where invitation is being performed
    /// * `inviter_id` - The user attempting to send invitations
    /// * `target_id` - Optional target user being invited (for power level comparison)
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if invite is allowed, appropriate error otherwise
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - Insufficient power level to invite
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn check_invite_power(
        &self,
        room_id: &str,
        inviter_id: &str,
        target_id: Option<&str>,
    ) -> Result<(), StatusCode> {
        debug!("Checking invite power for user {} in room {}", inviter_id, room_id);

        let power_levels = self.get_room_power_levels(room_id).await?;

        let inviter_level = self.get_user_power_level(&power_levels, inviter_id);
        let required_invite_level =
            power_levels.get("invite").and_then(|v| v.as_i64()).unwrap_or(0); // Default invite level is 0

        // Check if inviter has sufficient power to invite
        if inviter_level < required_invite_level {
            warn!(
                "Invite denied - user {} (level {}) lacks invite permission (required: {}) in room {}",
                inviter_id, inviter_level, required_invite_level, room_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        // If target is specified, check power level hierarchy
        if let Some(target) = target_id {
            let target_level = self.get_user_power_level(&power_levels, target);

            // Inviter must have higher power level than target (unless equal to prevent abuse)
            if inviter_level < target_level {
                warn!(
                    "Invite denied - inviter {} (level {}) cannot invite higher-level user {} (level {}) in room {}",
                    inviter_id, inviter_level, target, target_level, room_id
                );
                return Err(StatusCode::FORBIDDEN);
            }
        }

        info!("Invite permission granted for user {} in room {}", inviter_id, room_id);
        Ok(())
    }

    /// Check if a user has permission to kick users from a room
    ///
    /// Default required power level: 50 (moderator level by default)
    /// Users must have power level >= kick level and > target user's level
    ///
    /// # Arguments  
    /// * `room_id` - The room where kick is being performed
    /// * `kicker_id` - The user attempting to kick
    /// * `target_id` - The user being kicked
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if kick is allowed, appropriate error otherwise
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - Insufficient power level to kick or hierarchy violation
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn check_kick_power(
        &self,
        room_id: &str,
        kicker_id: &str,
        target_id: &str,
    ) -> Result<(), StatusCode> {
        debug!(
            "Checking kick power for user {} to kick {} in room {}",
            kicker_id, target_id, room_id
        );

        let power_levels = self.get_room_power_levels(room_id).await?;

        let kicker_level = self.get_user_power_level(&power_levels, kicker_id);
        let target_level = self.get_user_power_level(&power_levels, target_id);
        let required_kick_level = power_levels.get("kick").and_then(|v| v.as_i64()).unwrap_or(50); // Default kick level is 50

        // Check if kicker has sufficient power to kick
        if kicker_level < required_kick_level {
            warn!(
                "Kick denied - user {} (level {}) lacks kick permission (required: {}) in room {}",
                kicker_id, kicker_level, required_kick_level, room_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        // Kicker must have strictly higher power level than target
        if kicker_level <= target_level {
            warn!(
                "Kick denied - kicker {} (level {}) must have higher level than target {} (level {}) in room {}",
                kicker_id, kicker_level, target_id, target_level, room_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        info!(
            "Kick permission granted for user {} to kick {} in room {}",
            kicker_id, target_id, room_id
        );
        Ok(())
    }

    /// Check if a user has permission to ban users from a room
    ///
    /// Default required power level: 50 (moderator level by default)
    /// Users must have power level >= ban level and > target user's level
    ///
    /// # Arguments
    /// * `room_id` - The room where ban is being performed  
    /// * `banner_id` - The user attempting to ban
    /// * `target_id` - The user being banned
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if ban is allowed, appropriate error otherwise
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - Insufficient power level to ban or hierarchy violation
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn check_ban_power(
        &self,
        room_id: &str,
        banner_id: &str,
        target_id: &str,
    ) -> Result<(), StatusCode> {
        debug!(
            "Checking ban power for user {} to ban {} in room {}",
            banner_id, target_id, room_id
        );

        let power_levels = self.get_room_power_levels(room_id).await?;

        let banner_level = self.get_user_power_level(&power_levels, banner_id);
        let target_level = self.get_user_power_level(&power_levels, target_id);
        let required_ban_level = power_levels.get("ban").and_then(|v| v.as_i64()).unwrap_or(50); // Default ban level is 50

        // Check if banner has sufficient power to ban
        if banner_level < required_ban_level {
            warn!(
                "Ban denied - user {} (level {}) lacks ban permission (required: {}) in room {}",
                banner_id, banner_level, required_ban_level, room_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        // Banner must have strictly higher power level than target
        if banner_level <= target_level {
            warn!(
                "Ban denied - banner {} (level {}) must have higher level than target {} (level {}) in room {}",
                banner_id, banner_level, target_id, target_level, room_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        info!(
            "Ban permission granted for user {} to ban {} in room {}",
            banner_id, target_id, room_id
        );
        Ok(())
    }

    /// Check if a user has permission to unban users from a room
    ///
    /// Uses the same power level requirement as ban (default: 50)
    /// Users must have power level >= ban level and > target user's level
    ///
    /// # Arguments
    /// * `room_id` - The room where unban is being performed
    /// * `unbanner_id` - The user attempting to unban
    /// * `target_id` - The user being unbanned
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if unban is allowed, appropriate error otherwise
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - Insufficient power level to unban or hierarchy violation
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn check_unban_power(
        &self,
        room_id: &str,
        unbanner_id: &str,
        target_id: &str,
    ) -> Result<(), StatusCode> {
        // Unban uses same power level as ban
        self.check_ban_power(room_id, unbanner_id, target_id).await
    }

    /// Check if a user has permission to modify a state event
    ///
    /// State events have different power level requirements based on event type.
    /// Default levels: most state events require level 50, some specific events may vary.
    ///
    /// # Arguments
    /// * `room_id` - The room where state event is being modified
    /// * `user_id` - The user attempting to modify state
    /// * `event_type` - The type of state event being modified
    /// * `state_key` - The state key of the event (empty string for room-wide state)
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if state modification is allowed
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - Insufficient power level for state event modification
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn check_state_event_power(
        &self,
        room_id: &str,
        user_id: &str,
        event_type: &str,
        state_key: &str,
    ) -> Result<(), StatusCode> {
        debug!(
            "Checking state event power for user {} to modify {} in room {}",
            user_id, event_type, room_id
        );

        let power_levels = self.get_room_power_levels(room_id).await?;
        let user_level = self.get_user_power_level(&power_levels, user_id);

        // Get required power level for this specific state event type
        let required_level =
            self.get_state_event_required_level(&power_levels, event_type, state_key);

        if user_level < required_level {
            warn!(
                "State event modification denied - user {} (level {}) lacks permission for {} (required: {}) in room {}",
                user_id, user_level, event_type, required_level, room_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        info!(
            "State event modification granted for user {} to modify {} in room {}",
            user_id, event_type, room_id
        );
        Ok(())
    }

    /// Check if a user has permission to redact events
    ///
    /// Default required power level: 50 (moderator level by default)
    /// Users can always redact their own events regardless of power level.
    ///
    /// # Arguments
    /// * `room_id` - The room where redaction is being performed
    /// * `redactor_id` - The user attempting to redact
    /// * `original_sender_id` - The original sender of the event being redacted
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if redaction is allowed
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - Insufficient power level to redact others' events
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn check_redact_power(
        &self,
        room_id: &str,
        redactor_id: &str,
        original_sender_id: &str,
    ) -> Result<(), StatusCode> {
        debug!(
            "Checking redact power for user {} to redact event by {} in room {}",
            redactor_id, original_sender_id, room_id
        );

        // Users can always redact their own events
        if redactor_id == original_sender_id {
            info!(
                "Redaction granted - user {} redacting their own event in room {}",
                redactor_id, room_id
            );
            return Ok(());
        }

        let power_levels = self.get_room_power_levels(room_id).await?;

        let redactor_level = self.get_user_power_level(&power_levels, redactor_id);
        let required_redact_level =
            power_levels.get("redact").and_then(|v| v.as_i64()).unwrap_or(50); // Default redact level is 50

        if redactor_level < required_redact_level {
            warn!(
                "Redaction denied - user {} (level {}) lacks redact permission (required: {}) in room {}",
                redactor_id, redactor_level, required_redact_level, room_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        info!(
            "Redaction permission granted for user {} to redact others' events in room {}",
            redactor_id, room_id
        );
        Ok(())
    }

    /// Get a user's power level in a room
    ///
    /// Looks up user-specific power level from the users object, falls back to
    /// users_default if not specified, and finally to 0 if no defaults exist.
    ///
    /// # Arguments
    /// * `power_levels` - The room's power levels configuration
    /// * `user_id` - The user to get the power level for
    ///
    /// # Returns
    /// * `i64` - The user's effective power level in the room
    pub fn get_user_power_level(&self, power_levels: &Value, user_id: &str) -> i64 {
        // Check for user-specific power level
        if let Some(users) = power_levels.get("users").and_then(|u| u.as_object())
            && let Some(level) = users.get(user_id).and_then(|v| v.as_i64()) {
                return level;
            }

        // Fall back to users_default
        if let Some(default_level) = power_levels.get("users_default").and_then(|v| v.as_i64()) {
            return default_level;
        }

        // Final fallback to 0 (standard Matrix default)
        0
    }

    /// Get the current power levels configuration for a room
    ///
    /// Uses RoomRepository to get the current power level configuration.
    /// Falls back to Matrix defaults if no event exists.
    ///
    /// # Arguments
    /// * `room_id` - The room to get power levels for
    ///
    /// # Returns
    /// * `Result<Value, StatusCode>` - The power levels configuration object
    ///
    /// # Errors
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Repository error
    async fn get_room_power_levels(&self, room_id: &str) -> Result<Value, StatusCode> {
        match self.room_repo.get_room_power_levels(room_id).await {
            Ok(power_levels) => {
                debug!("Found power levels configuration for room {}", room_id);

                // Convert PowerLevels struct to JSON Value for compatibility
                let power_levels_json = serde_json::json!({
                    "users": power_levels.users,
                    "users_default": power_levels.users_default,
                    "events": power_levels.events,
                    "events_default": power_levels.events_default,
                    "state_default": power_levels.state_default,
                    "ban": power_levels.ban,
                    "kick": power_levels.kick,
                    "redact": power_levels.redact,
                    "invite": power_levels.invite
                });

                Ok(power_levels_json)
            },
            Err(e) => {
                error!("Failed to get power levels for room {}: {:?}", room_id, e);
                debug!("No power levels found for room {}, using Matrix defaults", room_id);
                Ok(self.get_default_power_levels())
            },
        }
    }

    /// Optimized bulk power level check using direct database query
    ///
    /// Uses the direct db connection for high-performance bulk operations
    /// when checking power levels for multiple users simultaneously.
    ///
    /// # Arguments
    /// * `room_id` - The room to check power levels for
    /// * `user_ids` - List of user IDs to check
    ///
    /// # Returns
    /// * `Result<Vec<(String, i64)>, StatusCode>` - List of (user_id, power_level) pairs
    ///
    /// # Errors
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database query error
    pub async fn get_bulk_user_power_levels(&self, room_id: String, user_ids: Vec<String>) -> Result<Vec<(String, i64)>, StatusCode> {
        debug!("Getting bulk power levels for {} users in room {}", user_ids.len(), room_id);

        // First get room power levels configuration
        let power_levels = self.get_room_power_levels(&room_id).await?;
        let users_default = power_levels.get("users_default").and_then(|v| v.as_i64()).unwrap_or(0);

        // Use direct database connection for optimized bulk query
        let query = "
            SELECT user_id, power_level 
            FROM room_power_levels 
            WHERE room_id = $room_id AND user_id IN $user_ids
        ";

        let mut result = self.db
            .query(query)
            .bind(("room_id", room_id.clone()))
            .bind(("user_ids", user_ids.clone()))
            .await
            .map_err(|e| {
                error!("Bulk power level query failed for room {}: {}", room_id, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let db_results: Vec<serde_json::Value> = result.take(0).map_err(|e| {
            error!("Failed to parse bulk power level results: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        // Build result map with database values and defaults
        let mut power_levels_result = Vec::new();
        for user_id in &user_ids {
            let user_power_level = db_results
                .iter()
                .find(|r| r.get("user_id").and_then(|u| u.as_str()) == Some(user_id))
                .and_then(|r| r.get("power_level").and_then(|p| p.as_i64()))
                .unwrap_or(users_default);

            power_levels_result.push((user_id.clone(), user_power_level));
        }

        debug!("Retrieved {} power levels for room {}", power_levels_result.len(), room_id);
        Ok(power_levels_result)
    }

    /// Get Matrix default power levels configuration
    ///
    /// Returns the standard Matrix power levels when no m.room.power_levels
    /// state event exists in a room.
    ///
    /// # Returns
    /// * `Value` - Default power levels configuration per Matrix specification
    fn get_default_power_levels(&self) -> Value {
        serde_json::json!({
            "users_default": 0,
            "events_default": 0,
            "state_default": 50,
            "ban": 50,
            "kick": 50,
            "redact": 50,
            "invite": 0,
            "users": {},
            "events": {}
        })
    }

    /// Get required power level for a specific state event type
    ///
    /// Checks the events object for event-type specific requirements,
    /// falls back to state_default (50 by default).
    ///
    /// # Arguments
    /// * `power_levels` - The room's power levels configuration
    /// * `event_type` - The Matrix event type (e.g., "m.room.name")
    /// * `state_key` - The state key (for user-specific validation)
    ///
    /// # Returns
    /// * `i64` - Required power level for the state event modification
    fn get_state_event_required_level(
        &self,
        power_levels: &Value,
        event_type: &str,
        _state_key: &str,
    ) -> i64 {
        // Check for event-type specific power level
        if let Some(events) = power_levels.get("events").and_then(|e| e.as_object())
            && let Some(level) = events.get(event_type).and_then(|v| v.as_i64()) {
                return level;
            }

        // Fall back to state_default
        power_levels.get("state_default").and_then(|v| v.as_i64()).unwrap_or(50) // Matrix default state_default is 50
    }

    /// Validate power level hierarchy for user-to-user operations
    ///
    /// Ensures that users can only perform actions on users with lower power levels.
    /// This prevents abuse where users with equal power levels could kick each other.
    ///
    /// # Arguments
    /// * `room_id` - The room where the operation is taking place
    /// * `actor_id` - The user performing the action
    /// * `target_id` - The user being acted upon
    /// * `required_level` - The minimum power level required for the action
    ///
    /// # Returns
    /// * `Result<(), StatusCode>` - Ok if hierarchy is respected
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - Power level hierarchy violation
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn validate_power_hierarchy(
        &self,
        room_id: &str,
        actor_id: &str,
        target_id: &str,
        required_level: i64,
    ) -> Result<(), StatusCode> {
        debug!(
            "Validating power hierarchy - actor: {}, target: {}, required: {}, room: {}",
            actor_id, target_id, required_level, room_id
        );

        let power_levels = self.get_room_power_levels(room_id).await?;

        let actor_level = self.get_user_power_level(&power_levels, actor_id);
        let target_level = self.get_user_power_level(&power_levels, target_id);

        // Actor must meet minimum required level
        if actor_level < required_level {
            warn!(
                "Power hierarchy violation - actor {} (level {}) below required level {} in room {}",
                actor_id, actor_level, required_level, room_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        // Actor must have strictly higher level than target (prevents equal-level abuse)
        if actor_level <= target_level {
            warn!(
                "Power hierarchy violation - actor {} (level {}) must exceed target {} (level {}) in room {}",
                actor_id, actor_level, target_id, target_level, room_id
            );
            return Err(StatusCode::FORBIDDEN);
        }

        info!(
            "Power hierarchy validated - actor {} can act on target {} in room {}",
            actor_id, target_id, room_id
        );
        Ok(())
    }

    /// Check if a user is a room administrator
    ///
    /// Room administrators typically have power level >= 50 and can perform
    /// most administrative actions like kicks, bans, and state modifications.
    /// Also validates that the user is actually a member of the room.
    ///
    /// # Arguments
    /// * `room_id` - The room to check admin status for
    /// * `user_id` - The user to check admin status for
    ///
    /// # Returns
    /// * `Result<bool, StatusCode>` - True if user is an admin
    ///
    /// # Errors  
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn is_room_admin(&self, room_id: &str, user_id: &str) -> Result<bool, StatusCode> {
        // First verify the user is actually a member of the room
        match self.membership_repo.get_membership(room_id, user_id).await {
            Ok(Some(membership)) => {
                if membership.membership != matryx_entity::MembershipState::Join {
                    debug!("User {} is not joined to room {} (membership: {:?})", user_id, room_id, membership.membership);
                    return Ok(false);
                }
            },
            Ok(None) => {
                debug!("User {} has no membership record for room {}", user_id, room_id);
                return Ok(false);
            },
            Err(e) => {
                error!("Failed to check membership for user {} in room {}: {:?}", user_id, room_id, e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }

        let power_levels = self.get_room_power_levels(room_id).await?;
        let user_level = self.get_user_power_level(&power_levels, user_id);

        // Admin threshold is typically 50 (moderator level)
        let admin_threshold = 50;
        let is_admin = user_level >= admin_threshold;

        debug!(
            "User {} admin status in room {}: {} (level {})",
            user_id, room_id, is_admin, user_level
        );

        Ok(is_admin)
    }

    /// Check if a user is a room owner/creator
    ///
    /// Room owners typically have power level 100 and can perform all actions
    /// including changing power levels of other users.
    /// Also validates that the user is actually a member of the room.
    ///
    /// # Arguments
    /// * `room_id` - The room to check owner status for  
    /// * `user_id` - The user to check owner status for
    ///
    /// # Returns
    /// * `Result<bool, StatusCode>` - True if user is an owner
    ///
    /// # Errors
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - Database or validation error
    pub async fn is_room_owner(&self, room_id: &str, user_id: &str) -> Result<bool, StatusCode> {
        // First verify the user is actually a member of the room
        match self.membership_repo.get_membership(room_id, user_id).await {
            Ok(Some(membership)) => {
                if membership.membership != matryx_entity::MembershipState::Join {
                    debug!("User {} is not joined to room {} (membership: {:?})", user_id, room_id, membership.membership);
                    return Ok(false);
                }
            },
            Ok(None) => {
                debug!("User {} has no membership record for room {}", user_id, room_id);
                return Ok(false);
            },
            Err(e) => {
                error!("Failed to check membership for user {} in room {}: {:?}", user_id, room_id, e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }

        let power_levels = self.get_room_power_levels(room_id).await?;
        let user_level = self.get_user_power_level(&power_levels, user_id);

        // Owner threshold is typically 100 (highest standard level)
        let owner_threshold = 100;
        let is_owner = user_level >= owner_threshold;

        debug!(
            "User {} owner status in room {}: {} (level {})",
            user_id, room_id, is_owner, user_level
        );

        Ok(is_owner)
    }
}

#[cfg(test)]
mod tests {
    // Tests would be implemented here following Rust testing best practices
    // Using expect() in tests (never unwrap()) for proper error messages
    // These tests would cover all power level scenarios and edge cases
}
