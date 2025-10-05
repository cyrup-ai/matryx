use crate::repository::error::RepositoryError;
use crate::repository::{
    CrossSigningRepository,
    CryptoRepository,
    DeviceRepository,
    KeyBackupRepository,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};

// Re-export types from sub-repositories
pub use crate::repository::cross_signing::{CrossSigningKey, CrossSigningKeys, Signature};
pub use crate::repository::crypto::{DeviceKey, FallbackKey, OneTimeKey};
pub use crate::repository::key_backup::{BackupStatistics, BackupVersion, EncryptedRoomKey};

// Import canonical DeviceKeys from entity package
use matryx_entity::types::DeviceKeys;

// Extended device keys struct for crypto service operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoDeviceKeys {
    pub device_keys: DeviceKeys,
    pub one_time_keys: HashMap<String, OneTimeKey>,
    pub fallback_keys: HashMap<String, FallbackKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimKeysRequest {
    pub one_time_keys: HashMap<String, HashMap<String, String>>, // user_id -> device_id -> algorithm
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimKeysResponse {
    pub one_time_keys: HashMap<String, HashMap<String, OneTimeKey>>, // user_id -> device_id -> key
    pub failures: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryKeysRequest {
    pub device_keys: HashMap<String, Vec<String>>, // user_id -> device_ids
    pub timeout: Option<u64>,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryKeysResponse {
    pub device_keys: HashMap<String, HashMap<String, DeviceKey>>, // user_id -> device_id -> device_key
    pub failures: HashMap<String, serde_json::Value>,
    pub master_keys: HashMap<String, CrossSigningKey>,
    pub self_signing_keys: HashMap<String, CrossSigningKey>,
    pub user_signing_keys: HashMap<String, CrossSigningKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedKeys {
    pub signatures: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureVerification {
    pub valid: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeyBackup {
    pub room_id: String,
    pub session_id: String,
    pub key_data: EncryptedRoomKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub count: u64,
    pub etag: String,
}

#[derive(Clone)]
pub struct CryptoService {
    crypto_repo: CryptoRepository,
    cross_signing_repo: CrossSigningRepository,
    key_backup_repo: KeyBackupRepository,
    _device_repo: DeviceRepository,
}

impl CryptoService {
    pub fn new(db: Surreal<Any>) -> Self {
        Self {
            crypto_repo: CryptoRepository::new(db.clone()),
            cross_signing_repo: CrossSigningRepository::new(db.clone()),
            key_backup_repo: KeyBackupRepository::new(db.clone()),
            _device_repo: DeviceRepository::new(db),
        }
    }

    pub async fn setup_cross_signing(
        &self,
        user_id: &str,
        keys: &CrossSigningKeys,
        device_id: &str,
    ) -> Result<(), RepositoryError> {
        // Store the cross-signing keys
        if let Some(ref master_key) = keys.master_key {
            self.cross_signing_repo.store_master_key(user_id, master_key).await?;
        }

        if let Some(ref self_signing_key) = keys.self_signing_key {
            self.cross_signing_repo
                .store_self_signing_key(user_id, self_signing_key)
                .await?;
        }

        if let Some(ref user_signing_key) = keys.user_signing_key {
            self.cross_signing_repo
                .store_user_signing_key(user_id, user_signing_key)
                .await?;
        }

        // Mark the device as trusted since it's setting up cross-signing
        self.cross_signing_repo
            .mark_device_trusted(user_id, device_id, user_id)
            .await?;

        Ok(())
    }

    pub async fn upload_device_keys(
        &self,
        user_id: &str,
        device_id: &str,
        keys: &CryptoDeviceKeys,
    ) -> Result<(), RepositoryError> {
        // Store the main device key
        let device_key = DeviceKey {
            user_id: keys.device_keys.user_id.clone(),
            device_id: keys.device_keys.device_id.clone(),
            algorithms: keys.device_keys.algorithms.clone(),
            keys: keys.device_keys.keys.clone(),
            signatures: keys.device_keys.signatures.clone(),
            unsigned: keys.device_keys.unsigned.as_ref()
                .and_then(|u| serde_json::to_value(u).ok()),
        };

        self.crypto_repo.store_device_key(user_id, device_id, &device_key).await?;

        // Store one-time keys
        for (key_id, one_time_key) in &keys.one_time_keys {
            self.crypto_repo
                .store_one_time_key(user_id, device_id, key_id, one_time_key)
                .await?;
        }

        // Store fallback keys
        for fallback_key in keys.fallback_keys.values() {
            self.crypto_repo
                .store_fallback_key(user_id, device_id, fallback_key)
                .await?;
        }

        Ok(())
    }

    pub async fn upload_one_time_keys(
        &self,
        user_id: &str,
        device_id: &str,
        keys: &HashMap<String, OneTimeKey>,
    ) -> Result<(), RepositoryError> {
        for (key_id, one_time_key) in keys {
            self.crypto_repo
                .store_one_time_key(user_id, device_id, key_id, one_time_key)
                .await?;
        }

        Ok(())
    }

    pub async fn claim_keys(
        &self,
        one_time_keys: &HashMap<String, HashMap<String, String>>,
    ) -> Result<ClaimKeysResponse, RepositoryError> {
        let mut response_keys = HashMap::new();
        let mut failures = HashMap::new();

        for (user_id, device_keys) in one_time_keys {
            let mut user_keys = HashMap::new();

            for (device_id, algorithm) in device_keys {
                match self.crypto_repo.claim_one_time_key(user_id, device_id, algorithm).await {
                    Ok(Some(key)) => {
                        user_keys.insert(device_id.clone(), key);
                    },
                    Ok(None) => {
                        // Try to get fallback key if no one-time key available
                        match self.crypto_repo.get_fallback_key(user_id, device_id).await {
                            Ok(Some(fallback_key)) => {
                                let one_time_key = OneTimeKey {
                                    key_id: fallback_key.key_id,
                                    key: fallback_key.key,
                                    algorithm: fallback_key.algorithm,
                                    signatures: fallback_key.signatures,
                                    created_at: fallback_key.created_at,
                                };
                                user_keys.insert(device_id.clone(), one_time_key);
                            },
                            Ok(None) => {
                                failures.insert(
                                    format!("{}:{}", user_id, device_id),
                                    serde_json::json!({"error": "No keys available"}),
                                );
                            },
                            Err(e) => {
                                failures.insert(
                                    format!("{}:{}", user_id, device_id),
                                    serde_json::json!({"error": e.to_string()}),
                                );
                            },
                        }
                    },
                    Err(e) => {
                        failures.insert(
                            format!("{}:{}", user_id, device_id),
                            serde_json::json!({"error": e.to_string()}),
                        );
                    },
                }
            }

            if !user_keys.is_empty() {
                response_keys.insert(user_id.clone(), user_keys);
            }
        }

        Ok(ClaimKeysResponse { one_time_keys: response_keys, failures })
    }

    pub async fn query_keys(
        &self,
        device_keys: &HashMap<String, Vec<String>>,
    ) -> Result<QueryKeysResponse, RepositoryError> {
        let mut response_device_keys = HashMap::new();
        let mut failures = HashMap::new();
        let mut master_keys = HashMap::new();
        let mut self_signing_keys = HashMap::new();
        let mut user_signing_keys = HashMap::new();

        for (user_id, device_ids) in device_keys {
            // Get device keys
            let mut user_device_keys = HashMap::new();

            if device_ids.is_empty() {
                // Get all device keys for the user
                match self.crypto_repo.get_user_device_keys(user_id).await {
                    Ok(keys) => {
                        user_device_keys.extend(keys);
                    },
                    Err(e) => {
                        failures
                            .insert(user_id.clone(), serde_json::json!({"error": e.to_string()}));
                    },
                }
            } else {
                // Get specific device keys
                for device_id in device_ids {
                    match self.crypto_repo.get_device_key(user_id, device_id).await {
                        Ok(Some(key)) => {
                            user_device_keys.insert(device_id.clone(), key);
                        },
                        Ok(None) => {
                            // Device key not found - not necessarily an error
                        },
                        Err(e) => {
                            failures.insert(
                                format!("{}:{}", user_id, device_id),
                                serde_json::json!({"error": e.to_string()}),
                            );
                        },
                    }
                }
            }

            if !user_device_keys.is_empty() {
                response_device_keys.insert(user_id.clone(), user_device_keys);
            }

            // Get cross-signing keys
            if let Ok(Some(master_key)) = self.cross_signing_repo.get_master_key(user_id).await {
                master_keys.insert(user_id.clone(), master_key);
            }

            if let Ok(Some(self_signing_key)) =
                self.cross_signing_repo.get_self_signing_key(user_id).await
            {
                self_signing_keys.insert(user_id.clone(), self_signing_key);
            }

            if let Ok(Some(user_signing_key)) =
                self.cross_signing_repo.get_user_signing_key(user_id).await
            {
                user_signing_keys.insert(user_id.clone(), user_signing_key);
            }
        }

        Ok(QueryKeysResponse {
            device_keys: response_device_keys,
            failures,
            master_keys,
            self_signing_keys,
            user_signing_keys,
        })
    }

    pub async fn sign_keys(
        &self,
        user_id: &str,
        keys: &serde_json::Value,
    ) -> Result<SignedKeys, RepositoryError> {
        // Get the user's self-signing key
        let self_signing_key = self
            .cross_signing_repo
            .get_self_signing_key(user_id)
            .await?
            .ok_or_else(|| {
                RepositoryError::NotFound {
                    entity_type: "self_signing_key".to_string(),
                    id: user_id.to_string(),
                }
            })?;

        // Extract the signing key
        let ed25519_key = self_signing_key
            .keys
            .iter()
            .find(|(k, _)| k.starts_with("ed25519:"))
            .ok_or_else(|| {
                RepositoryError::Validation {
                    field: "self_signing_key".to_string(),
                    message: "No ed25519 key found".to_string(),
                }
            })?;

        // Generate signature
        let signature = self.crypto_repo.generate_key_signature(keys, ed25519_key.1).await?;

        let mut signatures = HashMap::new();
        let mut user_signatures = HashMap::new();
        user_signatures.insert(format!("ed25519:{}", signature.key_id), signature.signature);
        signatures.insert(user_id.to_string(), user_signatures);

        Ok(SignedKeys { signatures })
    }

    pub async fn verify_key_signatures(
        &self,
        keys: &serde_json::Value,
        device_id: &str,
    ) -> Result<SignatureVerification, RepositoryError> {
        let mut errors = Vec::new();

        // Extract signatures from the keys object
        if let Some(signatures_obj) = keys.get("signatures").and_then(|s| s.as_object()) {
            for (user_id, user_sigs) in signatures_obj {
                if let Some(user_sigs_obj) = user_sigs.as_object() {
                    for (key_id, signature_str) in user_sigs_obj {
                        if let Some(sig_str) = signature_str.as_str() {
                            // Get the signing key (try both self-signing and device keys)
                            let signing_key_result = if let Ok(Some(self_signing_key)) =
                                self.cross_signing_repo.get_self_signing_key(user_id).await
                            {
                                let cross_signing_signature =
                                    crate::repository::cross_signing::Signature {
                                        signature: sig_str.to_string(),
                                        key_id: key_id
                                            .split(':')
                                            .nth(1)
                                            .unwrap_or("unknown")
                                            .to_string(),
                                        algorithm: key_id
                                            .split(':')
                                            .next()
                                            .unwrap_or("unknown")
                                            .to_string(),
                                    };
                                self.cross_signing_repo
                                    .validate_cross_signing_signature(
                                        &cross_signing_signature,
                                        &self_signing_key,
                                    )
                                    .await
                            } else if let Ok(Some(device_key)) =
                                self.crypto_repo.get_device_key(user_id, device_id).await
                            {
                                let crypto_signature = crate::repository::crypto::Signature {
                                    signature: sig_str.to_string(),
                                    key_id: key_id
                                            .split(':')
                                            .nth(1)
                                            .unwrap_or("unknown")
                                            .to_string(),
                                    algorithm: key_id
                                        .split(':')
                                        .next()
                                        .unwrap_or("unknown")
                                        .to_string(),
                                };
                                match device_key.keys.get(key_id) {
                                    Some(signing_key) => {
                                        self.crypto_repo
                                            .validate_key_signature(
                                                keys,
                                                &crypto_signature,
                                                signing_key,
                                            )
                                            .await
                                    },
                                    None => {
                                        errors.push(format!(
                                            "Signing key {} not found for device {}",
                                            key_id, device_id
                                        ));
                                        Ok(false)
                                    }
                                }
                            } else {
                                errors.push(format!(
                                    "Device key not found for user {} device {}",
                                    user_id, device_id
                                ));
                                Ok(false)
                            };

                            match signing_key_result {
                                Ok(false) => {
                                    errors.push(format!(
                                        "Invalid signature for {} by {}",
                                        key_id, user_id
                                    ));
                                },
                                Err(e) => {
                                    errors.push(format!(
                                        "Error verifying signature for {} by {}: {}",
                                        key_id, user_id, e
                                    ));
                                },
                                Ok(true) => {
                                    // Signature is valid
                                },
                            }
                        }
                    }
                }
            }
        }

        Ok(SignatureVerification { valid: errors.is_empty(), errors })
    }

    pub async fn create_key_backup(
        &self,
        user_id: &str,
        algorithm: &str,
        auth_data: &serde_json::Value,
    ) -> Result<String, RepositoryError> {
        self.key_backup_repo
            .create_backup_version(user_id, algorithm, auth_data)
            .await
    }

    pub async fn backup_room_keys(
        &self,
        user_id: &str,
        version: &str,
        keys: &[RoomKeyBackup],
    ) -> Result<BackupResult, RepositoryError> {
        let key_tuples: Vec<(String, String, EncryptedRoomKey)> = keys
            .iter()
            .map(|k| (k.room_id.clone(), k.session_id.clone(), k.key_data.clone()))
            .collect();

        let count = self
            .key_backup_repo
            .backup_room_keys_batch(user_id, version, &key_tuples)
            .await?;

        // Get updated backup version to return etag
        let backup_version = self
            .key_backup_repo
            .get_backup_version(user_id, version)
            .await?
            .ok_or_else(|| {
                RepositoryError::NotFound {
                    entity_type: "backup_version".to_string(),
                    id: version.to_string(),
                }
            })?;

        Ok(BackupResult { count, etag: backup_version.etag })
    }

    pub async fn restore_room_keys(
        &self,
        user_id: &str,
        version: &str,
        room_id: Option<&str>,
    ) -> Result<Vec<EncryptedRoomKey>, RepositoryError> {
        self.key_backup_repo.get_room_keys(user_id, version, room_id).await
    }

    pub async fn get_backup_info(
        &self,
        user_id: &str,
        version: &str,
    ) -> Result<(BackupVersion, BackupStatistics), RepositoryError> {
        let backup_version = self
            .key_backup_repo
            .get_backup_version(user_id, version)
            .await?
            .ok_or_else(|| {
                RepositoryError::NotFound {
                    entity_type: "backup_version".to_string(),
                    id: version.to_string(),
                }
            })?;

        let statistics = self.key_backup_repo.get_backup_statistics(user_id, version).await?;

        Ok((backup_version, statistics))
    }

    pub async fn delete_device_crypto_data(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<(), RepositoryError> {
        // Delete device keys and one-time keys
        self.crypto_repo.delete_device_keys(user_id, device_id).await?;

        // Remove device trust
        self.cross_signing_repo.revoke_device_trust(user_id, device_id).await?;

        Ok(())
    }

    pub async fn verify_cross_signing_setup(&self, user_id: &str) -> Result<bool, RepositoryError> {
        let keys = self.cross_signing_repo.get_all_cross_signing_keys(user_id).await?;

        // Check that all required keys are present
        Ok(keys.master_key.is_some() && keys.self_signing_key.is_some())
    }

    pub async fn get_device_key_count(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<HashMap<String, u32>, RepositoryError> {
        self.crypto_repo.get_one_time_key_count(user_id, device_id).await
    }

    pub async fn cleanup_old_crypto_data(&self, cutoff_days: u32) -> Result<u64, RepositoryError> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(cutoff_days as i64);
        self.crypto_repo.cleanup_expired_keys(cutoff).await
    }
}
