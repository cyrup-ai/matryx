use crate::repository::error::RepositoryError;
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSigningKey {
    pub user_id: String,
    pub usage: Vec<String>,
    pub keys: HashMap<String, String>,
    pub signatures: Option<HashMap<String, HashMap<String, String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSigningKeys {
    pub master_key: Option<CrossSigningKey>,
    pub self_signing_key: Option<CrossSigningKey>,
    pub user_signing_key: Option<CrossSigningKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceKey {
    pub user_id: String,
    pub device_id: String,
    pub algorithms: Vec<String>,
    pub keys: HashMap<String, String>,
    pub signatures: HashMap<String, HashMap<String, String>>,
    pub unsigned: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub signature: String,
    pub key_id: String,
    pub algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSigningKeyRecord {
    pub id: String,
    pub user_id: String,
    pub key_type: String, // "master", "self_signing", "user_signing"
    pub key_data: CrossSigningKey,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceTrust {
    pub id: String,
    pub user_id: String,
    pub device_id: String,
    pub trusted_by: String,
    pub trusted_at: DateTime<Utc>,
    pub trust_level: String, // "verified", "cross_signed", "manually_verified"
}

#[derive(Clone)]
pub struct CrossSigningRepository {
    db: Surreal<Any>,
}

impl CrossSigningRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn store_master_key(
        &self,
        user_id: &str,
        key: &CrossSigningKey,
    ) -> Result<(), RepositoryError> {
        let record = CrossSigningKeyRecord {
            id: format!("cross_signing_key:{}:master", user_id),
            user_id: user_id.to_string(),
            key_type: "master".to_string(),
            key_data: key.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let _: Option<CrossSigningKeyRecord> = self
            .db
            .create(("cross_signing_key", format!("{}:master", user_id)))
            .content(record)
            .await?;

        Ok(())
    }

    pub async fn store_self_signing_key(
        &self,
        user_id: &str,
        key: &CrossSigningKey,
    ) -> Result<(), RepositoryError> {
        let record = CrossSigningKeyRecord {
            id: format!("cross_signing_key:{}:self_signing", user_id),
            user_id: user_id.to_string(),
            key_type: "self_signing".to_string(),
            key_data: key.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let _: Option<CrossSigningKeyRecord> = self
            .db
            .create(("cross_signing_key", format!("{}:self_signing", user_id)))
            .content(record)
            .await?;

        Ok(())
    }

    pub async fn store_user_signing_key(
        &self,
        user_id: &str,
        key: &CrossSigningKey,
    ) -> Result<(), RepositoryError> {
        let record = CrossSigningKeyRecord {
            id: format!("cross_signing_key:{}:user_signing", user_id),
            user_id: user_id.to_string(),
            key_type: "user_signing".to_string(),
            key_data: key.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let _: Option<CrossSigningKeyRecord> = self
            .db
            .create(("cross_signing_key", format!("{}:user_signing", user_id)))
            .content(record)
            .await?;

        Ok(())
    }

    pub async fn get_master_key(
        &self,
        user_id: &str,
    ) -> Result<Option<CrossSigningKey>, RepositoryError> {
        let record: Option<CrossSigningKeyRecord> = self
            .db
            .select(("cross_signing_key", format!("{}:master", user_id)))
            .await?;

        Ok(record.map(|r| r.key_data))
    }

    pub async fn get_self_signing_key(
        &self,
        user_id: &str,
    ) -> Result<Option<CrossSigningKey>, RepositoryError> {
        let record: Option<CrossSigningKeyRecord> = self
            .db
            .select(("cross_signing_key", format!("{}:self_signing", user_id)))
            .await?;

        Ok(record.map(|r| r.key_data))
    }

    pub async fn get_user_signing_key(
        &self,
        user_id: &str,
    ) -> Result<Option<CrossSigningKey>, RepositoryError> {
        let record: Option<CrossSigningKeyRecord> = self
            .db
            .select(("cross_signing_key", format!("{}:user_signing", user_id)))
            .await?;

        Ok(record.map(|r| r.key_data))
    }

    pub async fn get_all_cross_signing_keys(
        &self,
        user_id: &str,
    ) -> Result<CrossSigningKeys, RepositoryError> {
        let master_key = self.get_master_key(user_id).await?;
        let self_signing_key = self.get_self_signing_key(user_id).await?;
        let user_signing_key = self.get_user_signing_key(user_id).await?;

        Ok(CrossSigningKeys { master_key, self_signing_key, user_signing_key })
    }

    pub async fn validate_cross_signing_signature(
        &self,
        signature: &Signature,
        signing_key: &CrossSigningKey,
    ) -> Result<bool, RepositoryError> {
        // Extract the public key for the specified algorithm
        let public_key = signing_key
            .keys
            .get(&format!("{}:{}", signature.algorithm, signature.key_id))
            .ok_or_else(|| {
                RepositoryError::Validation {
                    field: "signing_key".to_string(),
                    message: format!(
                        "No key found for algorithm {} and key_id {}",
                        signature.algorithm, signature.key_id
                    ),
                }
            })?;

        // For ed25519 signatures, we need to validate against the canonical JSON
        if signature.algorithm == "ed25519" {
            // Decode the signature from base64
            let signature_bytes =
                general_purpose::STANDARD.decode(&signature.signature).map_err(|e| {
                    RepositoryError::Validation {
                        field: "signature".to_string(),
                        message: format!("Invalid base64 signature: {}", e),
                    }
                })?;

            // Decode the public key from base64
            let public_key_bytes = general_purpose::STANDARD.decode(public_key).map_err(|e| {
                RepositoryError::Validation {
                    field: "public_key".to_string(),
                    message: format!("Invalid base64 public key: {}", e),
                }
            })?;

            // In a real implementation, this would use proper ed25519 verification
            // For now, we'll validate the format and structure
            if signature_bytes.len() == 64 && public_key_bytes.len() == 32 {
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Err(RepositoryError::Validation {
                field: "algorithm".to_string(),
                message: format!("Unsupported signature algorithm: {}", signature.algorithm),
            })
        }
    }

    pub async fn sign_device_key(
        &self,
        user_id: &str,
        _device_id: &str,
        device_key: &DeviceKey,
        signing_key: &CrossSigningKey,
    ) -> Result<Signature, RepositoryError> {
        // Validate that this is a self-signing key
        if !signing_key.usage.contains(&"self_signing".to_string()) {
            return Err(RepositoryError::Validation {
                field: "signing_key".to_string(),
                message: "Key must have self_signing usage".to_string(),
            });
        }

        // Get the ed25519 signing key
        let ed25519_key = signing_key
            .keys
            .iter()
            .find(|(k, _)| k.starts_with("ed25519:"))
            .ok_or_else(|| {
                RepositoryError::Validation {
                    field: "signing_key".to_string(),
                    message: "No ed25519 key found in signing key".to_string(),
                }
            })?;

        // Create canonical JSON of device key (without signatures)
        let mut device_value =
            serde_json::to_value(device_key).map_err(RepositoryError::Serialization)?;

        if let Some(obj) = device_value.as_object_mut() {
            obj.remove("signatures");
            obj.remove("unsigned");
        }

        let canonical_json =
            serde_json::to_string(&device_value).map_err(RepositoryError::Serialization)?;

        // In a real implementation, this would use actual ed25519 signing
        // For now, we'll create a mock signature
        let signature_data = format!("{}:{}:{}", canonical_json, ed25519_key.1, user_id);
        let mock_signature = general_purpose::STANDARD.encode(signature_data.as_bytes());

        Ok(Signature {
            signature: mock_signature,
            key_id: ed25519_key.0.split(':').nth(1).unwrap_or("unknown").to_string(),
            algorithm: "ed25519".to_string(),
        })
    }

    pub async fn verify_device_signature(
        &self,
        user_id: &str,
        _device_id: &str,
        signature: &Signature,
    ) -> Result<bool, RepositoryError> {
        // Get the self-signing key for the user
        let self_signing_key = self.get_self_signing_key(user_id).await?.ok_or_else(|| {
            RepositoryError::NotFound {
                entity_type: "self_signing_key".to_string(),
                id: user_id.to_string(),
            }
        })?;

        // Validate the signature using the self-signing key
        self.validate_cross_signing_signature(signature, &self_signing_key).await
    }

    pub async fn get_trusted_devices(&self, user_id: &str) -> Result<Vec<String>, RepositoryError> {
        let query = "SELECT device_id FROM device_trust WHERE user_id = $user_id AND trust_level IN ['verified', 'cross_signed']";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let trusted: Vec<DeviceTrust> = result.take(0)?;
        Ok(trusted.into_iter().map(|t| t.device_id).collect())
    }

    pub async fn mark_device_trusted(
        &self,
        user_id: &str,
        device_id: &str,
        trusted_by: &str,
    ) -> Result<(), RepositoryError> {
        let trust_record = DeviceTrust {
            id: format!("device_trust:{}:{}", user_id, device_id),
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            trusted_by: trusted_by.to_string(),
            trusted_at: Utc::now(),
            trust_level: "cross_signed".to_string(),
        };

        let _: Option<DeviceTrust> = self
            .db
            .create(("device_trust", format!("{}:{}", user_id, device_id)))
            .content(trust_record)
            .await?;

        Ok(())
    }

    pub async fn revoke_device_trust(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<(), RepositoryError> {
        let _: Option<DeviceTrust> = self
            .db
            .delete(("device_trust", format!("{}:{}", user_id, device_id)))
            .await?;

        Ok(())
    }

    pub async fn delete_cross_signing_keys(&self, user_id: &str) -> Result<(), RepositoryError> {
        // Delete all cross-signing keys for the user
        let _: Option<CrossSigningKeyRecord> = self
            .db
            .delete(("cross_signing_key", format!("{}:master", user_id)))
            .await?;

        let _: Option<CrossSigningKeyRecord> = self
            .db
            .delete(("cross_signing_key", format!("{}:self_signing", user_id)))
            .await?;

        let _: Option<CrossSigningKeyRecord> = self
            .db
            .delete(("cross_signing_key", format!("{}:user_signing", user_id)))
            .await?;

        Ok(())
    }

    pub async fn get_cross_signing_key_by_type(
        &self,
        user_id: &str,
        key_type: &str,
    ) -> Result<Option<CrossSigningKey>, RepositoryError> {
        let record: Option<CrossSigningKeyRecord> = self
            .db
            .select(("cross_signing_key", format!("{}:{}", user_id, key_type)))
            .await?;

        Ok(record.map(|r| r.key_data))
    }

    pub async fn update_cross_signing_key(
        &self,
        user_id: &str,
        key_type: &str,
        key: &CrossSigningKey,
    ) -> Result<(), RepositoryError> {
        let record_id = format!("{}:{}", user_id, key_type);

        // Get existing record to preserve created_at
        let existing: Option<CrossSigningKeyRecord> =
            self.db.select(("cross_signing_key", &record_id)).await?;

        let created_at = existing.map(|r| r.created_at).unwrap_or_else(Utc::now);

        let record = CrossSigningKeyRecord {
            id: format!("cross_signing_key:{}", record_id),
            user_id: user_id.to_string(),
            key_type: key_type.to_string(),
            key_data: key.clone(),
            created_at,
            updated_at: Utc::now(),
        };

        let _: Option<CrossSigningKeyRecord> =
            self.db.update(("cross_signing_key", record_id)).content(record).await?;

        Ok(())
    }
}
