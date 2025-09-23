use crate::repository::error::RepositoryError;

use matryx_entity::types::{User, UserInfo, UserProfile};
use surrealdb::{Surreal, engine::any::Any};

#[derive(Clone)]
pub struct UserRepository {
    db: Surreal<Any>,
}

impl UserRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create(&self, user: &User) -> Result<User, RepositoryError> {
        let user_clone = user.clone();
        let created: Option<User> =
            self.db.create(("user", &user.user_id)).content(user_clone).await?;
        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create user"))
        })
    }

    pub async fn get_by_id(&self, user_id: &str) -> Result<Option<User>, RepositoryError> {
        let user: Option<User> = self.db.select(("user", user_id)).await?;
        Ok(user)
    }

    pub async fn update(&self, user: &User) -> Result<User, RepositoryError> {
        let user_clone = user.clone();
        let updated: Option<User> =
            self.db.update(("user", &user.user_id)).content(user_clone).await?;
        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update user"))
        })
    }

    pub async fn delete(&self, user_id: &str) -> Result<(), RepositoryError> {
        let _: Option<User> = self.db.delete(("user", user_id)).await?;
        Ok(())
    }

    pub async fn authenticate(
        &self,
        user_id: &str,
        password_hash: &str,
    ) -> Result<Option<User>, RepositoryError> {
        let query = "SELECT * FROM user WHERE user_id = $user_id AND password_hash = $password_hash AND is_active = true LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("password_hash", password_hash.to_string()))
            .await?;
        let users: Vec<User> = result.take(0)?;
        Ok(users.into_iter().next())
    }

    pub async fn get_all_users(&self, limit: Option<i64>) -> Result<Vec<User>, RepositoryError> {
        let query = match limit {
            Some(l) => format!("SELECT * FROM user LIMIT {}", l),
            None => "SELECT * FROM user".to_string(),
        };
        let mut result = self.db.query(&query).await?;
        let users: Vec<User> = result.take(0)?;
        Ok(users)
    }

    /// Check if a user exists
    pub async fn user_exists(&self, user_id: &str) -> Result<bool, RepositoryError> {
        let query = "SELECT count() FROM user WHERE user_id = $user_id GROUP ALL";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    /// Check if a user is active
    pub async fn is_user_active(&self, user_id: &str) -> Result<bool, RepositoryError> {
        let query = "SELECT is_active FROM user WHERE user_id = $user_id LIMIT 1";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let users: Vec<serde_json::Value> = result.take(0)?;

        if let Some(user) = users.first() &&
            let Some(is_active) = user.get("is_active").and_then(|v| v.as_bool())
        {
            return Ok(is_active);
        }

        Ok(false)
    }

    /// Validate user for room joining
    pub async fn validate_user_for_join(&self, user_id: &str) -> Result<bool, RepositoryError> {
        // Check if user exists and is active
        let exists = self.user_exists(user_id).await?;
        if !exists {
            return Ok(false);
        }

        let is_active = self.is_user_active(user_id).await?;
        Ok(is_active)
    }

    /// Get user's power level in a room
    pub async fn get_user_power_level(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<i32, RepositoryError> {
        let query = "
            SELECT content FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.power_levels'
            AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let events: Vec<serde_json::Value> = result.take(0)?;

        if let Some(event) = events.first() {
            if let Some(content) = event.get("content") {
                // Check if user has specific power level
                if let Some(users) = content.get("users").and_then(|v| v.as_object()) {
                    if let Some(power_level) = users.get(user_id).and_then(|v| v.as_i64()) {
                        return Ok(power_level as i32);
                    }
                }

                // Return default power level
                if let Some(default_level) = content.get("users_default").and_then(|v| v.as_i64()) {
                    return Ok(default_level as i32);
                }
            }
        }

        // Default Matrix power level for regular users
        Ok(0)
    }

    /// Check if user can join a room
    pub async fn can_user_join_room(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<bool, RepositoryError> {
        // First validate the user
        if !self.validate_user_for_join(user_id).await? {
            return Ok(false);
        }

        // Check if user is already in the room
        let membership_query = "
            SELECT membership FROM membership 
            WHERE room_id = $room_id AND user_id = $user_id 
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(membership_query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;
        let memberships: Vec<serde_json::Value> = result.take(0)?;

        if let Some(membership) = memberships.first() {
            if let Some(state) = membership.get("membership").and_then(|v| v.as_str()) {
                match state {
                    "join" => return Ok(false),  // Already joined
                    "ban" => return Ok(false),   // Banned from room
                    "invite" => return Ok(true), // Has invitation
                    "knock" => return Ok(true),  // Has knock request
                    _ => {},                     // Continue with other checks
                }
            }
        }

        // Check room join rules
        let join_rules_query = "
            SELECT content FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.join_rules'
            AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(join_rules_query)
            .bind(("room_id", room_id.to_string()))
            .await?;
        let events: Vec<serde_json::Value> = result.take(0)?;

        if let Some(event) = events.first() {
            if let Some(content) = event.get("content") {
                if let Some(join_rule) = content.get("join_rule").and_then(|v| v.as_str()) {
                    match join_rule {
                        "public" => return Ok(true),
                        "invite" => return Ok(false), // Need invitation (already checked above)
                        "knock" => return Ok(false),  // Need to knock first
                        "restricted" => {
                            // Check if user is member of allowed rooms (simplified)
                            return Ok(false);
                        },
                        _ => return Ok(false),
                    }
                }
            }
        }

        // Default to invite-only if no join rules found
        Ok(false)
    }

    /// Get user profile information
    pub async fn get_user_profile(&self, user_id: &str) -> Result<UserProfile, RepositoryError> {
        let user = self.get_by_id(user_id).await?.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "User".to_string(),
                id: user_id.to_string(),
            }
        })?;

        Ok(UserProfile::with_profile_data(user.user_id, user.display_name, user.avatar_url))
    }

    /// Update user display name
    pub async fn update_display_name(
        &self,
        user_id: &str,
        display_name: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Validate display name length (Matrix spec: max 256 characters)
        if let Some(ref name) = display_name {
            if name.len() > 256 {
                return Err(RepositoryError::Validation {
                    field: "display_name".to_string(),
                    message: "Display name must not exceed 256 characters".to_string(),
                });
            }
        }

        let query = "UPDATE user SET display_name = $display_name WHERE user_id = $user_id";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("display_name", display_name))
            .await?;

        let updated: Vec<User> = result.take(0)?;
        if updated.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "User".to_string(),
                id: user_id.to_string(),
            });
        }

        Ok(())
    }

    /// Get user display name
    pub async fn get_display_name(&self, user_id: &str) -> Result<Option<String>, RepositoryError> {
        let query = "SELECT display_name FROM user WHERE user_id = $user_id LIMIT 1";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let users: Vec<serde_json::Value> = result.take(0)?;
        if let Some(user) = users.first() {
            let display_name =
                user.get("display_name").and_then(|v| v.as_str()).map(|s| s.to_string());
            return Ok(display_name);
        }

        Err(RepositoryError::NotFound {
            entity_type: "User".to_string(),
            id: user_id.to_string(),
        })
    }

    /// Update user avatar URL
    pub async fn update_avatar_url(
        &self,
        user_id: &str,
        avatar_url: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Validate avatar URL format if provided
        if let Some(ref url) = avatar_url
            && !url.starts_with("mxc://") {
            return Err(RepositoryError::Validation {
                field: "avatar_url".to_string(),
                message: "Avatar URL must use mxc:// scheme".to_string(),
            });
        }

        let query = "UPDATE user SET avatar_url = $avatar_url WHERE user_id = $user_id";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("avatar_url", avatar_url))
            .await?;

        let updated: Vec<User> = result.take(0)?;
        if updated.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "User".to_string(),
                id: user_id.to_string(),
            });
        }

        Ok(())
    }

    /// Get user avatar URL
    pub async fn get_avatar_url(&self, user_id: &str) -> Result<Option<String>, RepositoryError> {
        let query = "SELECT avatar_url FROM user WHERE user_id = $user_id LIMIT 1";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let users: Vec<serde_json::Value> = result.take(0)?;
        if let Some(user) = users.first() {
            let avatar_url = user.get("avatar_url").and_then(|v| v.as_str()).map(|s| s.to_string());
            return Ok(avatar_url);
        }

        Err(RepositoryError::NotFound {
            entity_type: "User".to_string(),
            id: user_id.to_string(),
        })
    }

    /// Deactivate user account
    pub async fn deactivate_account(
        &self,
        user_id: &str,
        erase_data: bool,
    ) -> Result<(), RepositoryError> {
        if erase_data {
            // Erase user profile data
            let query = "UPDATE user SET is_active = false, display_name = NONE, avatar_url = NONE WHERE user_id = $user_id";
            let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

            let updated: Vec<User> = result.take(0)?;
            if updated.is_empty() {
                return Err(RepositoryError::NotFound {
                    entity_type: "User".to_string(),
                    id: user_id.to_string(),
                });
            }
        } else {
            // Just deactivate the account
            let query = "UPDATE user SET is_active = false WHERE user_id = $user_id";
            let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

            let updated: Vec<User> = result.take(0)?;
            if updated.is_empty() {
                return Err(RepositoryError::NotFound {
                    entity_type: "User".to_string(),
                    id: user_id.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Get user information for whoami endpoint
    pub async fn get_user_info(&self, user_id: &str) -> Result<UserInfo, RepositoryError> {
        let user = self.get_by_id(user_id).await?.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "User".to_string(),
                id: user_id.to_string(),
            }
        })?;

        Ok(UserInfo::from_user(&user))
    }

    /// Validate profile update permissions
    pub async fn validate_profile_update_permissions(
        &self,
        user_id: &str,
        requesting_user: &str,
    ) -> Result<bool, RepositoryError> {
        // User can always update their own profile
        if user_id == requesting_user {
            return Ok(true);
        }

        // Check if requesting user is admin
        let query = "SELECT is_admin FROM user WHERE user_id = $requesting_user LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("requesting_user", requesting_user.to_string()))
            .await?;

        let users: Vec<serde_json::Value> = result.take(0)?;
        if let Some(user) = users.first()
            && let Some(is_admin) = user.get("is_admin").and_then(|v| v.as_bool()) {
            return Ok(is_admin);
        }

        // Default to no permission
        Ok(false)
    }

    /// Get user display name from profile
    pub async fn get_user_display_name(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let query = "SELECT display_name FROM user_profiles WHERE user_id = $user_id";
        let mut response = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let display_name: Option<String> = response.take(0)?;
        Ok(display_name)
    }

    /// Get user avatar URL from profile
    pub async fn get_user_avatar_url(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let query = "SELECT avatar_url FROM user_profiles WHERE user_id = $user_id";
        let mut response = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let avatar_url: Option<String> = response.take(0)?;
        Ok(avatar_url)
    }
}
