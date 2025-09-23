use crate::repository::error::RepositoryError;
use chrono::{Duration, Utc};
use matryx_entity::types::ThirdPartyId;
use surrealdb::{Connection, Surreal};
use uuid::Uuid;

#[derive(Clone)]
pub struct ThirdPartyRepository<C: Connection> {
    db: Surreal<C>,
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
        if let Some(existing_user) = self.find_user_by_third_party(medium, address).await? {
            if existing_user != user_id {
                return Err(RepositoryError::Conflict {
                    message: "Third-party identifier already exists for another user".to_string(),
                });
            }
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
        if let Some(row) = rows.first() {
            if let Some(user_id) = row.get("user_id").and_then(|v| v.as_str()) {
                return Ok(Some(user_id.to_string()));
            }
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
        if let Some(row) = rows.first() {
            if let Some(validated) = row.get("validated").and_then(|v| v.as_bool()) {
                return Ok(validated);
            }
        }

        Err(RepositoryError::NotFound {
            entity_type: "ThirdPartyIdentifier".to_string(),
            id: format!("{}:{}:{}", user_id, medium, address),
        })
    }
}
