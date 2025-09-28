use crate::repository::error::RepositoryError;
use chrono::{DateTime, Duration, Utc};
use matryx_entity::types::ThirdPartyId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};
use uuid::Uuid;

// TASK17 SUBTASK 9: Add supporting types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyProtocol {
    pub protocol_id: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub user_fields: Vec<FieldType>,
    pub location_fields: Vec<FieldType>,
    pub instances: Vec<ProtocolInstance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolInstance {
    pub instance_id: String,
    pub desc: String,
    pub icon: Option<String>,
    pub fields: HashMap<String, String>,
    pub network_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyLocation {
    pub alias: String,
    pub protocol: String,
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartyUser {
    pub userid: String,
    pub protocol: String,
    pub fields: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    pub bridge_id: String,
    pub protocol: String,
    pub name: String,
    pub url: String,
    pub as_token: String,
    pub hs_token: String,
    pub status: BridgeStatus,
    pub created_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeStatus {
    Active,
    Inactive,
    Error,
    Maintenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeStatistics {
    pub total_users: u64,
    pub total_rooms: u64,
    pub messages_bridged_24h: u64,
    pub uptime_percentage: f64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldType {
    pub regexp: String,
    pub placeholder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolStatistics {
    pub protocol_id: String,
    pub total_bridges: u64,
    pub active_bridges: u64,
    pub total_users: u64,
    pub total_locations: u64,
    pub messages_24h: u64,
    pub uptime_percentage: f64,
}

#[derive(Clone)]
pub struct ThirdPartyRepository<C: Connection> {
    pub(crate) db: Surreal<C>,
}

impl<C: Connection> ThirdPartyRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Add a third-party identifier for a user
    pub async fn add_third_party_identifier(
        &self,
        user_id: &str,
        medium: &str,
        address: &str,
        validated: bool,
    ) -> Result<ThirdPartyId, RepositoryError> {
        // Validate parameters
        if medium.is_empty() {
            return Err(RepositoryError::Validation {
                field: "medium".to_string(),
                message: "Medium cannot be empty".to_string(),
            });
        }

        if address.is_empty() {
            return Err(RepositoryError::Validation {
                field: "address".to_string(),
                message: "Address cannot be empty".to_string(),
            });
        }

        // Validate medium type
        if !matches!(medium, "email" | "msisdn") {
            return Err(RepositoryError::Validation {
                field: "medium".to_string(),
                message: "Medium must be 'email' or 'msisdn'".to_string(),
            });
        }

        // Check if identifier already exists for another user
        if let Some(existing_user) = self.find_user_by_third_party(medium, address).await?
            && existing_user != user_id {
            return Err(RepositoryError::Conflict {
                message: "Third-party identifier already exists for another user".to_string(),
            });
        }

        let third_party_id = Uuid::new_v4().to_string();
        let identifier = if validated {
            ThirdPartyId::new(
                third_party_id.clone(),
                user_id.to_string(),
                medium.to_string(),
                address.to_string(),
                true,
            )
        } else {
            // Generate validation token for unvalidated identifiers
            let validation_token = Uuid::new_v4().to_string();
            let expires_at = Utc::now() + Duration::hours(24); // 24 hour expiry
            ThirdPartyId::with_validation_token(
                third_party_id.clone(),
                user_id.to_string(),
                medium.to_string(),
                address.to_string(),
                validation_token,
                expires_at,
            )
        };

        let identifier_content = identifier.clone();
        let created: Option<ThirdPartyId> = self
            .db
            .create(("third_party_identifiers", &third_party_id))
            .content(identifier_content)
            .await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg(
                "Failed to create third-party identifier",
            ))
        })
    }
    /// Get all third-party identifiers for a user
    pub async fn get_user_third_party_ids(
        &self,
        user_id: &str,
    ) -> Result<Vec<ThirdPartyId>, RepositoryError> {
        let query = "SELECT * FROM third_party_identifiers WHERE user_id = $user_id ORDER BY created_at DESC";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let identifiers: Vec<ThirdPartyId> = result.take(0)?;
        Ok(identifiers)
    }

    /// Remove a third-party identifier
    pub async fn remove_third_party_identifier(
        &self,
        user_id: &str,
        medium: &str,
        address: &str,
    ) -> Result<(), RepositoryError> {
        let query = "DELETE FROM third_party_identifiers WHERE user_id = $user_id AND medium = $medium AND address = $address";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("medium", medium.to_string()))
            .bind(("address", address.to_string()))
            .await?;

        let deleted: Vec<ThirdPartyId> = result.take(0)?;
        if deleted.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "ThirdPartyIdentifier".to_string(),
                id: format!("{}:{}:{}", user_id, medium, address),
            });
        }

        Ok(())
    }

    /// Validate a third-party identifier using a token
    pub async fn validate_third_party_identifier(
        &self,
        user_id: &str,
        medium: &str,
        address: &str,
        token: &str,
    ) -> Result<bool, RepositoryError> {
        // Find the identifier
        let query = "SELECT * FROM third_party_identifiers WHERE user_id = $user_id AND medium = $medium AND address = $address LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("medium", medium.to_string()))
            .bind(("address", address.to_string()))
            .await?;

        let identifiers: Vec<ThirdPartyId> = result.take(0)?;
        let mut identifier = identifiers.into_iter().next().ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "ThirdPartyIdentifier".to_string(),
                id: format!("{}:{}:{}", user_id, medium, address),
            }
        })?;

        // Check if already validated
        if identifier.validated {
            return Ok(true);
        }

        // Check token
        if let Some(validation_token) = &identifier.validation_token {
            if validation_token != token {
                return Ok(false);
            }

            // Check if token is expired
            if identifier.is_token_expired() {
                return Ok(false);
            }

            // Mark as validated
            identifier.validate();

            // Update in database
            let update_query = r#"
                UPDATE third_party_identifiers SET
                    validated = true,
                    validated_at = time::now(),
                    updated_at = time::now(),
                    validation_token = NONE,
                    token_expires_at = NONE
                WHERE user_id = $user_id AND medium = $medium AND address = $address
            "#;

            let mut update_result = self
                .db
                .query(update_query)
                .bind(("user_id", user_id.to_string()))
                .bind(("medium", medium.to_string()))
                .bind(("address", address.to_string()))
                .await?;

            let _: Vec<ThirdPartyId> = update_result.take(0)?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Find user by third-party identifier
    pub async fn find_user_by_third_party(
        &self,
        medium: &str,
        address: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let query = "SELECT user_id FROM third_party_identifiers WHERE medium = $medium AND address = $address AND validated = true LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("medium", medium.to_string()))
            .bind(("address", address.to_string()))
            .await?;

        let rows: Vec<serde_json::Value> = result.take(0)?;
        if let Some(row) = rows.first()
            && let Some(user_id) = row.get("user_id").and_then(|v| v.as_str()) {
            return Ok(Some(user_id.to_string()));
        }

        Ok(None)
    }

    /// Get validation status for a third-party identifier
    pub async fn get_third_party_validation_status(
        &self,
        user_id: &str,
        medium: &str,
        address: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "SELECT validated FROM third_party_identifiers WHERE user_id = $user_id AND medium = $medium AND address = $address LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("medium", medium.to_string()))
            .bind(("address", address.to_string()))
            .await?;

        let rows: Vec<serde_json::Value> = result.take(0)?;
        if let Some(row) = rows.first()
            && let Some(validated) = row.get("validated").and_then(|v| v.as_bool()) {
                return Ok(validated);
            }

        Err(RepositoryError::NotFound {
            entity_type: "ThirdPartyIdentifier".to_string(),
            id: format!("{}:{}:{}", user_id, medium, address),
        })
    }

    // TASK17 SUBTASK 2: Add missing methods to ThirdPartyRepository

    /// Get all available third-party protocols
    pub async fn get_all_protocols(&self) -> Result<Vec<ThirdPartyProtocol>, RepositoryError> {
        let query = "SELECT * FROM thirdparty_protocols ORDER BY protocol_id";
        let mut result = self.db.query(query).await.map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_all_protocols".to_string(),
        })?;

        let protocols: Vec<ThirdPartyProtocol> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_all_protocols_parse".to_string(),
        })?;

        Ok(protocols)
    }

    /// Get a specific protocol by ID
    pub async fn get_protocol_by_id(&self, protocol_id: &str) -> Result<Option<ThirdPartyProtocol>, RepositoryError> {
        let query = "SELECT * FROM thirdparty_protocols WHERE protocol_id = $protocol_id LIMIT 1";
        let mut result = self.db
            .query(query)
            .bind(("protocol_id", protocol_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_protocol_by_id".to_string(),
            })?;

        let protocols: Vec<ThirdPartyProtocol> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_protocol_by_id_parse".to_string(),
        })?;

        Ok(protocols.into_iter().next())
    }

    /// Get protocol instances for a specific protocol
    pub async fn get_protocol_instances(&self, protocol_id: &str) -> Result<Vec<ProtocolInstance>, RepositoryError> {
        let query = "SELECT instances FROM thirdparty_protocols WHERE protocol_id = $protocol_id LIMIT 1";
        let mut result = self.db
            .query(query)
            .bind(("protocol_id", protocol_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_protocol_instances".to_string(),
            })?;

        let rows: Vec<serde_json::Value> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_protocol_instances_parse".to_string(),
        })?;

        if let Some(row) = rows.first()
            && let Some(instances_value) = row.get("instances") {
                let instances: Vec<ProtocolInstance> = serde_json::from_value(instances_value.clone())
                    .map_err(|e| RepositoryError::DatabaseError {
                        message: format!("Failed to parse protocol instances: {}", e),
                        operation: "get_protocol_instances_deserialize".to_string(),
                    })?;
                return Ok(instances);
            }

        Ok(Vec::new())
    }

    /// Lookup third-party locations by protocol and search fields
    #[allow(dead_code)]
    pub async fn lookup_third_party_location(&self, _protocol: &str, _search_fields: &HashMap<String, String>) -> Result<Vec<ThirdPartyLocation>, RepositoryError> {
        // TODO: Fix lifetime issues
        Ok(Vec::new())
    }


    /// Lookup third-party users by protocol and search fields
    #[allow(dead_code)]
    pub async fn lookup_third_party_user(&self, _protocol: &str, _search_fields: &HashMap<String, String>) -> Result<Vec<ThirdPartyUser>, RepositoryError> {
        // TODO: Fix lifetime issues
        Ok(Vec::new())
    }


    /// Get location by alias
    pub async fn get_location_by_alias(&self, alias: &str) -> Result<Option<ThirdPartyLocation>, RepositoryError> {
        let query = "SELECT * FROM thirdparty_locations WHERE alias = $alias LIMIT 1";
        let mut result = self.db
            .query(query)
            .bind(("alias", alias.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_location_by_alias".to_string(),
            })?;

        let locations: Vec<ThirdPartyLocation> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_location_by_alias_parse".to_string(),
        })?;

        Ok(locations.into_iter().next())
    }

    /// Get user by userid
    pub async fn get_user_by_userid(&self, userid: &str) -> Result<Option<ThirdPartyUser>, RepositoryError> {
        let query = "SELECT * FROM thirdparty_users WHERE userid = $userid LIMIT 1";
        let mut result = self.db
            .query(query)
            .bind(("userid", userid.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_user_by_userid".to_string(),
            })?;

        let users: Vec<ThirdPartyUser> = result.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_user_by_userid_parse".to_string(),
        })?;

        Ok(users.into_iter().next())
    }

    /// Register a new third-party protocol
    pub async fn register_protocol(&self, protocol: &ThirdPartyProtocol) -> Result<(), RepositoryError> {
        let _: Option<ThirdPartyProtocol> = self.db
            .create(("thirdparty_protocols", &protocol.protocol_id))
            .content(protocol.clone())
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "register_protocol".to_string(),
            })?;

        Ok(())
    }

    /// Update protocol instances for a specific protocol
    pub async fn update_protocol_instances(&self, protocol_id: &str, instances: &[ProtocolInstance]) -> Result<(), RepositoryError> {
        let query = "UPDATE thirdparty_protocols SET instances = $instances WHERE protocol_id = $protocol_id";
        self.db
            .query(query)
            .bind(("protocol_id", protocol_id.to_string()))
            .bind(("instances", instances.to_vec()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "update_protocol_instances".to_string(),
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    include!("third_party_tests.rs");
}
