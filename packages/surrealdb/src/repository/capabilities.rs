use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};

use crate::repository::RepositoryError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesResponse {
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub change_password: bool,
    pub room_versions: RoomVersionCapabilities,
    pub set_displayname: bool,
    pub set_avatar_url: bool,
    pub threepid_changes: bool,
    pub get_login_token: bool,
    pub lazy_loading: bool,
    pub e2e_cross_signing: bool,
    pub spaces: bool,
    pub threading: bool,
    pub custom_capabilities: HashMap<String, Value>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomVersionCapabilities {
    pub default: String,
    pub available: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomCapabilities {
    pub version: String,
    pub features: Vec<String>,
    pub state_resolution: String,
    pub supported_event_types: Vec<String>,
    pub room_state_default: HashMap<String, i32>,
    pub room_state_events: HashMap<String, i32>,
    pub room_events_default: i32,
    pub room_events: HashMap<String, i32>,
    pub room_ban: i32,
    pub room_kick: i32,
    pub room_redact: i32,
    pub room_invite: i32,
    pub room_state_default_power: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCapabilities {
    pub user_id: String,
    pub can_change_password: bool,
    pub can_set_displayname: bool,
    pub can_set_avatar_url: bool,
    pub can_create_rooms: bool,
    pub can_join_public_rooms: bool,
    pub can_invite_users: bool,
    pub max_upload_size: u64,
    pub rate_limits: HashMap<String, RateLimit>,
    pub custom_capabilities: HashMap<String, Value>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub requests_per_second: f64,
    pub burst_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityExtension {
    pub extension_id: String,
    pub name: String,
    pub version: String,
    pub capabilities: HashMap<String, Value>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

pub struct CapabilitiesRepository {
    db: Surreal<Any>,
}

impl CapabilitiesRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn get_server_capabilities(&self) -> Result<ServerCapabilities, RepositoryError> {
        let capabilities_query =
            "SELECT * FROM server_capabilities ORDER BY updated_at DESC LIMIT 1";

        let mut response = self
            .db
            .query(capabilities_query)
            .await
            .map_err(RepositoryError::Database)?;

        let capabilities_data: Vec<Value> = response.take(0).map_err(RepositoryError::Database)?;

        if let Some(capabilities_value) = capabilities_data.into_iter().next() {
            serde_json::from_value(capabilities_value).map_err(RepositoryError::Serialization)
        } else {
            // Return default capabilities if none exist
            Ok(self.get_default_server_capabilities())
        }
    }

    pub async fn get_room_capabilities(
        &self,
        room_version: &str,
    ) -> Result<RoomCapabilities, RepositoryError> {
        let capabilities_query = "SELECT * FROM room_capabilities WHERE version = $version";

        let mut response = self
            .db
            .query(capabilities_query)
            .bind(("version", room_version.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        let capabilities_data: Vec<Value> = response.take(0).map_err(RepositoryError::Database)?;

        if let Some(capabilities_value) = capabilities_data.into_iter().next() {
            serde_json::from_value(capabilities_value).map_err(RepositoryError::Serialization)
        } else {
            // Return default capabilities for the room version
            Ok(self.get_default_room_capabilities(room_version))
        }
    }

    pub async fn get_user_capabilities(
        &self,
        user_id: &str,
    ) -> Result<UserCapabilities, RepositoryError> {
        let capabilities_query = "SELECT * FROM user_capabilities WHERE user_id = $user_id";

        let mut response = self
            .db
            .query(capabilities_query)
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "get_user_capabilities".to_string(),
                }
            })?;

        let capabilities_data: Vec<Value> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_user_capabilities_parse".to_string(),
            }
        })?;

        if let Some(capabilities_value) = capabilities_data.into_iter().next() {
            serde_json::from_value(capabilities_value).map_err(|e| {
                RepositoryError::SerializationError {
                    message: format!("Failed to deserialize user capabilities: {}", e),
                }
            })
        } else {
            // Return default capabilities for the user
            Ok(self.get_default_user_capabilities(user_id))
        }
    }

    pub async fn update_server_capabilities(
        &self,
        capabilities: &ServerCapabilities,
    ) -> Result<(), RepositoryError> {
        let update_query = r#"
            CREATE server_capabilities CONTENT $capabilities
        "#;

        self.db
            .query(update_query)
            .bind(("capabilities", capabilities.clone()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "update_server_capabilities".to_string(),
                }
            })?;

        Ok(())
    }

    pub async fn check_feature_support(&self, feature: &str) -> Result<bool, RepositoryError> {
        // Check if feature is supported by looking at capabilities
        let capabilities = self.get_server_capabilities().await?;

        let supported = match feature {
            "m.change_password" => capabilities.change_password,
            "m.set_displayname" => capabilities.set_displayname,
            "m.set_avatar_url" => capabilities.set_avatar_url,
            "m.3pid_changes" => capabilities.threepid_changes,
            "m.get_login_token" => capabilities.get_login_token,
            "org.matrix.lazy_loading" => capabilities.lazy_loading,
            "org.matrix.e2e_cross_signing" => capabilities.e2e_cross_signing,
            "org.matrix.spaces" => capabilities.spaces,
            "org.matrix.threading" => capabilities.threading,
            _ => {
                // Check custom capabilities
                capabilities
                    .custom_capabilities
                    .get(feature)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            },
        };

        Ok(supported)
    }

    pub async fn get_supported_room_versions(&self) -> Result<Vec<String>, RepositoryError> {
        let capabilities = self.get_server_capabilities().await?;
        Ok(capabilities.room_versions.available.keys().cloned().collect())
    }

    pub async fn get_unstable_features(&self) -> Result<HashMap<String, bool>, RepositoryError> {
        let unstable_query =
            "SELECT feature_name, enabled FROM unstable_features WHERE enabled = true";

        let mut response = self.db.query(unstable_query).await.map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_unstable_features".to_string(),
            }
        })?;

        let features_data: Vec<(String, bool)> = response.take(0).map_err(|e| {
            RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_unstable_features_parse".to_string(),
            }
        })?;

        Ok(features_data.into_iter().collect())
    }

    pub async fn register_capability_extension(
        &self,
        extension: &CapabilityExtension,
    ) -> Result<(), RepositoryError> {
        let register_query = r#"
            UPSERT capability_extensions:$extension_id CONTENT $extension
        "#;

        self.db
            .query(register_query)
            .bind(("extension_id", extension.extension_id.clone()))
            .bind(("extension", extension.clone()))
            .await
            .map_err(|e| {
                RepositoryError::DatabaseError {
                    message: e.to_string(),
                    operation: "register_capability_extension".to_string(),
                }
            })?;

        Ok(())
    }

    // Helper methods for default capabilities

    fn get_default_server_capabilities(&self) -> ServerCapabilities {
        let mut available_versions = HashMap::new();
        available_versions.insert("1".to_string(), "stable".to_string());
        available_versions.insert("2".to_string(), "stable".to_string());
        available_versions.insert("3".to_string(), "stable".to_string());
        available_versions.insert("4".to_string(), "stable".to_string());
        available_versions.insert("5".to_string(), "stable".to_string());
        available_versions.insert("6".to_string(), "stable".to_string());
        available_versions.insert("7".to_string(), "stable".to_string());
        available_versions.insert("8".to_string(), "stable".to_string());
        available_versions.insert("9".to_string(), "stable".to_string());
        available_versions.insert("10".to_string(), "stable".to_string());

        ServerCapabilities {
            change_password: true,
            room_versions: RoomVersionCapabilities {
                default: "9".to_string(),
                available: available_versions,
            },
            set_displayname: true,
            set_avatar_url: true,
            threepid_changes: true,
            get_login_token: false,
            lazy_loading: true,
            e2e_cross_signing: true,
            spaces: true,
            threading: true,
            custom_capabilities: HashMap::new(),
            updated_at: Utc::now(),
        }
    }

    fn get_default_room_capabilities(&self, room_version: &str) -> RoomCapabilities {
        let features = match room_version {
            "1" => vec!["basic_events".to_string()],
            "2" => {
                vec![
                    "basic_events".to_string(),
                    "state_resolution_v1".to_string(),
                ]
            },
            "3" | "4" | "5" | "6" => {
                vec![
                    "basic_events".to_string(),
                    "state_resolution_v2".to_string(),
                    "restricted_rooms".to_string(),
                ]
            },
            "7" | "8" | "9" | "10" => {
                vec![
                    "basic_events".to_string(),
                    "state_resolution_v2".to_string(),
                    "restricted_rooms".to_string(),
                    "knock_restricted_rooms".to_string(),
                ]
            },
            _ => vec!["basic_events".to_string()],
        };

        let supported_event_types = vec![
            "m.room.message".to_string(),
            "m.room.member".to_string(),
            "m.room.create".to_string(),
            "m.room.join_rules".to_string(),
            "m.room.power_levels".to_string(),
            "m.room.name".to_string(),
            "m.room.topic".to_string(),
            "m.room.avatar".to_string(),
            "m.room.canonical_alias".to_string(),
            "m.room.history_visibility".to_string(),
            "m.room.guest_access".to_string(),
            "m.room.encryption".to_string(),
        ];

        let mut room_state_events = HashMap::new();
        room_state_events.insert("m.room.power_levels".to_string(), 100);
        room_state_events.insert("m.room.join_rules".to_string(), 50);
        room_state_events.insert("m.room.history_visibility".to_string(), 100);

        let mut room_events = HashMap::new();
        room_events.insert("m.room.message".to_string(), 0);
        room_events.insert("m.reaction".to_string(), 0);

        RoomCapabilities {
            version: room_version.to_string(),
            features,
            state_resolution: if room_version == "1" || room_version == "2" {
                "1".to_string()
            } else {
                "2".to_string()
            },
            supported_event_types,
            room_state_default: HashMap::new(),
            room_state_events,
            room_events_default: 0,
            room_events,
            room_ban: 50,
            room_kick: 50,
            room_redact: 50,
            room_invite: 50,
            room_state_default_power: 50,
        }
    }

    fn get_default_user_capabilities(&self, user_id: &str) -> UserCapabilities {
        let mut rate_limits = HashMap::new();
        rate_limits.insert("messages".to_string(), RateLimit {
            requests_per_second: 10.0,
            burst_count: 50,
        });
        rate_limits
            .insert("login".to_string(), RateLimit { requests_per_second: 1.0, burst_count: 5 });

        UserCapabilities {
            user_id: user_id.to_string(),
            can_change_password: true,
            can_set_displayname: true,
            can_set_avatar_url: true,
            can_create_rooms: true,
            can_join_public_rooms: true,
            can_invite_users: true,
            max_upload_size: 50 * 1024 * 1024, // 50MB
            rate_limits,
            custom_capabilities: HashMap::new(),
            updated_at: Utc::now(),
        }
    }
}
