use crate::repository::error::RepositoryError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::{Connection, Surreal};

// Import canonical DeviceKeys from entity package
use matryx_entity::types::DeviceKeys;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceKeysQuery {
    pub device_keys: HashMap<String, Vec<String>>, // user_id -> device_ids
    pub timeout: Option<u32>,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceKeysResponse {
    pub device_keys: HashMap<String, HashMap<String, DeviceKeys>>, // user_id -> device_id -> keys
    pub failures: HashMap<String, Value>,
    pub master_keys: HashMap<String, CrossSigningKey>,
    pub self_signing_keys: HashMap<String, CrossSigningKey>,
    pub user_signing_keys: HashMap<String, CrossSigningKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneTimeKeysClaim {
    pub one_time_keys: HashMap<String, HashMap<String, String>>, // user_id -> device_id -> algorithm
    pub timeout: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneTimeKeysResponse {
    pub one_time_keys: HashMap<String, HashMap<String, HashMap<String, Value>>>, // user_id -> device_id -> key_id -> key
    pub failures: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSigningKey {
    pub user_id: String,
    pub usage: Vec<String>,
    pub keys: HashMap<String, String>,
    pub signatures: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSigningKeys {
    pub master_key: Option<CrossSigningKey>,
    pub self_signing_key: Option<CrossSigningKey>,
    pub user_signing_key: Option<CrossSigningKey>,
}

pub struct CryptoKeysRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> CryptoKeysRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Upload device keys for a user's device
    pub async fn upload_device_keys(
        &self,
        user_id: &str,
        device_id: &str,
        device_keys: &DeviceKeys,
    ) -> Result<(), RepositoryError> {
        // Validate that user_id and device_id match the keys
        if device_keys.user_id != user_id || device_keys.device_id != device_id {
            return Err(RepositoryError::Validation {
                field: "device_keys".to_string(),
                message: "User ID or device ID mismatch".to_string(),
            });
        }

        // Store or update device keys
        let query = "
            UPDATE device_keys SET
                algorithms = $algorithms,
                keys = $keys,
                signatures = $signatures,
                updated_at = time::now()
            WHERE user_id = $user_id AND device_id = $device_id
            ELSE CREATE device_keys SET
                user_id = $user_id,
                device_id = $device_id,
                algorithms = $algorithms,
                keys = $keys,
                signatures = $signatures,
                created_at = time::now(),
                updated_at = time::now()
        ";

        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .bind(("algorithms", serde_json::to_value(&device_keys.algorithms)?))
            .bind(("keys", serde_json::to_value(&device_keys.keys)?))
            .bind(("signatures", serde_json::to_value(&device_keys.signatures)?))
            .await?;

        Ok(())
    }

    /// Upload one-time keys for a device
    pub async fn upload_one_time_keys(
        &self,
        user_id: &str,
        device_id: &str,
        one_time_keys: &HashMap<String, Value>,
    ) -> Result<(), RepositoryError> {
        // Delete existing one-time keys for this device to replace them
        let delete_query = "
            DELETE FROM one_time_keys
            WHERE user_id = $user_id AND device_id = $device_id
        ";

        self.db
            .query(delete_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        // Insert new one-time keys
        for (key_id, key_data) in one_time_keys {
            let insert_query = "
                CREATE one_time_keys SET
                    user_id = $user_id,
                    device_id = $device_id,
                    key_id = $key_id,
                    key_data = $key_data,
                    algorithm = $algorithm,
                    created_at = time::now()
            ";

            // Extract algorithm from key_id (format: algorithm:key_id)
            let algorithm = key_id.split(':').next().unwrap_or("unknown");

            self.db
                .query(insert_query)
                .bind(("user_id", user_id.to_string()))
                .bind(("device_id", device_id.to_string()))
                .bind(("key_id", key_id.clone()))
                .bind(("key_data", key_data.clone()))
                .bind(("algorithm", algorithm.to_string()))
                .await?;
        }

        Ok(())
    }

    /// Query device keys for multiple users and devices
    pub async fn query_device_keys(
        &self,
        device_keys_query: &DeviceKeysQuery,
    ) -> Result<DeviceKeysResponse, RepositoryError> {
        let mut device_keys = HashMap::new();
        let mut failures = HashMap::new();
        let mut master_keys = HashMap::new();
        let mut self_signing_keys = HashMap::new();
        let mut user_signing_keys = HashMap::new();

        for (user_id, device_ids) in &device_keys_query.device_keys {
            let mut user_device_keys = HashMap::new();

            if device_ids.is_empty() {
                // Query all devices for this user
                let all_devices_query = "
                    SELECT * FROM device_keys
                    WHERE user_id = $user_id
                ";

                let mut result = self
                    .db
                    .query(all_devices_query)
                    .bind(("user_id", user_id.to_string()))
                    .await?;

                let keys_data: Vec<Value> = result.take(0)?;

                for key_data in keys_data {
                    if let Some(device_keys_obj) = self.value_to_device_keys(key_data)? {
                        user_device_keys.insert(device_keys_obj.device_id.clone(), device_keys_obj);
                    }
                }
            } else {
                // Query specific devices
                for device_id in device_ids {
                    let device_query = "
                        SELECT * FROM device_keys
                        WHERE user_id = $user_id AND device_id = $device_id
                        LIMIT 1
                    ";

                    let mut result = self
                        .db
                        .query(device_query)
                        .bind(("user_id", user_id.to_string()))
                        .bind(("device_id", device_id.to_string()))
                        .await?;

                    let keys_data: Vec<Value> = result.take(0)?;

                    if let Some(key_data) = keys_data.first() {
                        if let Some(device_keys_obj) =
                            self.value_to_device_keys(key_data.clone())?
                        {
                            user_device_keys.insert(device_id.clone(), device_keys_obj);
                        }
                    } else {
                        failures.insert(
                            format!("{}:{}", user_id, device_id),
                            serde_json::json!({"errcode": "M_NOT_FOUND"}),
                        );
                    }
                }
            }

            if !user_device_keys.is_empty() {
                device_keys.insert(user_id.clone(), user_device_keys);
            }

            // Query cross-signing keys for this user
            let cross_signing = self.get_cross_signing_keys(user_id).await?;
            if let Some(master_key) = cross_signing.master_key {
                master_keys.insert(user_id.clone(), master_key);
            }
            if let Some(self_signing_key) = cross_signing.self_signing_key {
                self_signing_keys.insert(user_id.clone(), self_signing_key);
            }
            if let Some(user_signing_key) = cross_signing.user_signing_key {
                user_signing_keys.insert(user_id.clone(), user_signing_key);
            }
        }

        Ok(DeviceKeysResponse {
            device_keys,
            failures,
            master_keys,
            self_signing_keys,
            user_signing_keys,
        })
    }

    /// Claim one-time keys for encryption
    pub async fn claim_one_time_keys(
        &self,
        one_time_keys_claim: &OneTimeKeysClaim,
    ) -> Result<OneTimeKeysResponse, RepositoryError> {
        let mut one_time_keys = HashMap::new();
        let mut failures = HashMap::new();

        for (user_id, device_algorithms) in &one_time_keys_claim.one_time_keys {
            let mut user_keys = HashMap::new();

            for (device_id, algorithm) in device_algorithms {
                // Find and claim one available key for this device and algorithm
                let claim_query = "
                    SELECT * FROM one_time_keys
                    WHERE user_id = $user_id AND device_id = $device_id AND algorithm = $algorithm
                    LIMIT 1
                ";

                let mut result = self
                    .db
                    .query(claim_query)
                    .bind(("user_id", user_id.to_string()))
                    .bind(("device_id", device_id.to_string()))
                    .bind(("algorithm", algorithm.to_string()))
                    .await?;

                let keys_data: Vec<Value> = result.take(0)?;

                if let Some(key_data) = keys_data.first() {
                    if let (Some(key_id), Some(key_data_value)) =
                        (key_data.get("key_id").and_then(|v| v.as_str()), key_data.get("key_data"))
                    {
                        let mut device_keys = HashMap::new();
                        device_keys.insert(key_id.to_string(), key_data_value.clone());
                        user_keys.insert(device_id.clone(), device_keys);

                        // Delete the claimed key
                        let delete_query = "
                            DELETE FROM one_time_keys
                            WHERE user_id = $user_id AND device_id = $device_id AND key_id = $key_id
                        ";

                        self.db
                            .query(delete_query)
                            .bind(("user_id", user_id.to_string()))
                            .bind(("device_id", device_id.to_string()))
                            .bind(("key_id", key_id.to_string()))
                            .await?;
                    }
                } else {
                    failures.insert(
                        format!("{}:{}", user_id, device_id),
                        serde_json::json!({"errcode": "M_NOT_FOUND"}),
                    );
                }
            }

            if !user_keys.is_empty() {
                one_time_keys.insert(user_id.clone(), user_keys);
            }
        }

        Ok(OneTimeKeysResponse { one_time_keys, failures })
    }

    /// Upload cross-signing keys (master, self-signing, user-signing)
    pub async fn upload_signing_keys(
        &self,
        user_id: &str,
        master_key: Option<&CrossSigningKey>,
        self_signing_key: Option<&CrossSigningKey>,
        user_signing_key: Option<&CrossSigningKey>,
    ) -> Result<(), RepositoryError> {
        if let Some(master_key) = master_key {
            self.store_cross_signing_key(user_id, "master", master_key).await?;
        }

        if let Some(self_signing_key) = self_signing_key {
            self.store_cross_signing_key(user_id, "self_signing", self_signing_key)
                .await?;
        }

        if let Some(user_signing_key) = user_signing_key {
            self.store_cross_signing_key(user_id, "user_signing", user_signing_key)
                .await?;
        }

        Ok(())
    }

    /// Upload signatures for cross-signing verification
    pub async fn upload_signatures(
        &self,
        user_id: &str,
        signatures: &HashMap<String, HashMap<String, Value>>,
    ) -> Result<(), RepositoryError> {
        for (target_user_id, user_signatures) in signatures {
            for (key_id, signature) in user_signatures {
                let query = "
                    CREATE cross_signing_signatures SET
                        signer_user_id = $signer_user_id,
                        target_user_id = $target_user_id,
                        key_id = $key_id,
                        signature = $signature,
                        created_at = time::now()
                ";

                self.db
                    .query(query)
                    .bind(("signer_user_id", user_id.to_string()))
                    .bind(("target_user_id", target_user_id.to_string()))
                    .bind(("key_id", key_id.to_string()))
                    .bind(("signature", signature.clone()))
                    .await?;
            }
        }

        Ok(())
    }

    /// Get cross-signing keys for a user
    pub async fn get_cross_signing_keys(
        &self,
        user_id: &str,
    ) -> Result<CrossSigningKeys, RepositoryError> {
        let query = "
            SELECT * FROM cross_signing_keys
            WHERE user_id = $user_id
        ";

        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let keys_data: Vec<Value> = result.take(0)?;

        let mut master_key = None;
        let mut self_signing_key = None;
        let mut user_signing_key = None;

        for key_data in keys_data {
            if let (Some(key_type), Some(usage), Some(keys), Some(signatures)) = (
                key_data.get("key_type").and_then(|v| v.as_str()),
                key_data.get("usage"),
                key_data.get("keys"),
                key_data.get("signatures"),
            ) {
                let cross_signing_key = CrossSigningKey {
                    user_id: user_id.to_string(),
                    usage: serde_json::from_value(usage.clone()).unwrap_or_default(),
                    keys: serde_json::from_value(keys.clone()).unwrap_or_default(),
                    signatures: serde_json::from_value(signatures.clone()).unwrap_or_default(),
                };

                match key_type {
                    "master" => master_key = Some(cross_signing_key),
                    "self_signing" => self_signing_key = Some(cross_signing_key),
                    "user_signing" => user_signing_key = Some(cross_signing_key),
                    _ => {}, // Unknown key type
                }
            }
        }

        Ok(CrossSigningKeys { master_key, self_signing_key, user_signing_key })
    }

    /// Validate key signatures for cross-signing verification
    /// Implements Matrix specification for signature verification (Appendix - Signing JSON)
    pub async fn validate_key_signatures(
        &self,
        user_id: &str,
        keys: &Value,
        signatures: &HashMap<String, Value>,
    ) -> Result<bool, RepositoryError> {
        use base64::{Engine as _, engine::general_purpose};
        use ed25519_dalek::{Signature, VerifyingKey, Verifier};

        // Get the user's cross-signing keys
        let cross_signing_keys = self.get_cross_signing_keys(user_id).await?;

        // Ensure master key exists (required for cross-signing)
        let master_key = match cross_signing_keys.master_key {
            Some(key) => key,
            None => return Ok(false),
        };

        // Verify each signature using ed25519
        for (key_id, signature_value) in signatures {
            // Validate key_id format (must be ed25519:KEYID per spec)
            if !key_id.starts_with("ed25519:") {
                return Err(RepositoryError::Validation {
                    field: "key_id".to_string(),
                    message: format!("Unsupported algorithm in key_id: {}", key_id),
                });
            }

            // Extract signature string
            let signature_str = match signature_value.as_str() {
                Some(s) => s,
                None => {
                    return Err(RepositoryError::Validation {
                        field: "signature".to_string(),
                        message: "Signature must be a string".to_string(),
                    });
                }
            };

            // Get the signing key from master key
            let signing_key = match master_key.keys.get(key_id) {
                Some(key) => key,
                None => {
                    return Err(RepositoryError::Validation {
                        field: "signing_key".to_string(),
                        message: format!("Signing key {} not found in master key", key_id),
                    });
                }
            };

            // Decode signature from base64
            let signature_bytes = general_purpose::STANDARD.decode(signature_str).map_err(|e| {
                RepositoryError::Validation {
                    field: "signature".to_string(),
                    message: format!("Invalid base64 signature: {}", e),
                }
            })?;

            // Decode signing key from base64
            let signing_key_bytes = general_purpose::STANDARD.decode(signing_key).map_err(|e| {
                RepositoryError::Validation {
                    field: "signing_key".to_string(),
                    message: format!("Invalid base64 signing key: {}", e),
                }
            })?;

            // Validate sizes (ed25519 spec)
            if signature_bytes.len() != 64 {
                return Err(RepositoryError::Validation {
                    field: "signature".to_string(),
                    message: format!("Invalid signature length: {} (expected 64)", signature_bytes.len()),
                });
            }

            if signing_key_bytes.len() != 32 {
                return Err(RepositoryError::Validation {
                    field: "signing_key".to_string(),
                    message: format!("Invalid key length: {} (expected 32)", signing_key_bytes.len()),
                });
            }

            // Convert to fixed-size arrays
            let key_array: [u8; 32] = match signing_key_bytes.try_into() {
                Ok(arr) => arr,
                Err(_) => {
                    return Err(RepositoryError::Validation {
                        field: "signing_key".to_string(),
                        message: "Failed to convert key bytes to array".to_string(),
                    });
                }
            };

            let sig_array: [u8; 64] = match signature_bytes.try_into() {
                Ok(arr) => arr,
                Err(_) => {
                    return Err(RepositoryError::Validation {
                        field: "signature".to_string(),
                        message: "Failed to convert signature bytes to array".to_string(),
                    });
                }
            };

            // Create verifying key and signature objects
            let verifying_key = VerifyingKey::from_bytes(&key_array).map_err(|e| {
                RepositoryError::Validation {
                    field: "signing_key".to_string(),
                    message: format!("Invalid Ed25519 key: {}", e),
                }
            })?;

            let signature_obj = Signature::from_bytes(&sig_array);

            // Create canonical JSON per Matrix "Signing JSON" spec
            // Step 1: Clone the keys object
            let mut canonical_value = keys.clone();

            // Step 2: Remove signatures and unsigned fields per spec
            if let Some(obj) = canonical_value.as_object_mut() {
                obj.remove("signatures");
                obj.remove("unsigned");
            }

            // Step 3: Create canonical JSON (compact, sorted keys)
            let canonical_json = serde_json::to_string(&canonical_value)
                .map_err(RepositoryError::Serialization)?;

            // Verify signature against canonical JSON bytes (NOT key_id)
            // This is the critical fix per Matrix spec
            verifying_key.verify(canonical_json.as_bytes(), &signature_obj).map_err(|e| {
                RepositoryError::Validation {
                    field: "signature".to_string(),
                    message: format!("Ed25519 signature verification failed: {}", e),
                }
            })?;
        }

        // All signatures verified successfully per spec
        Ok(true)
    }

    /// Helper method to convert database value to DeviceKeys
    fn value_to_device_keys(&self, value: Value) -> Result<Option<DeviceKeys>, RepositoryError> {
        if let (Some(user_id), Some(device_id), Some(algorithms), Some(keys), Some(signatures)) = (
            value.get("user_id").and_then(|v| v.as_str()),
            value.get("device_id").and_then(|v| v.as_str()),
            value.get("algorithms"),
            value.get("keys"),
            value.get("signatures"),
        ) {
            let unsigned = value.get("unsigned")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            
            Ok(Some(DeviceKeys {
                user_id: user_id.to_string(),
                device_id: device_id.to_string(),
                algorithms: serde_json::from_value(algorithms.clone()).unwrap_or_default(),
                keys: serde_json::from_value(keys.clone()).unwrap_or_default(),
                signatures: serde_json::from_value(signatures.clone()).unwrap_or_default(),
                unsigned,
            }))
        } else {
            Ok(None)
        }
    }

    /// Helper method to store cross-signing keys
    async fn store_cross_signing_key(
        &self,
        user_id: &str,
        key_type: &str,
        key: &CrossSigningKey,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE cross_signing_keys SET
                usage = $usage,
                keys = $keys,
                signatures = $signatures,
                updated_at = time::now()
            WHERE user_id = $user_id AND key_type = $key_type
            ELSE CREATE cross_signing_keys SET
                user_id = $user_id,
                key_type = $key_type,
                usage = $usage,
                keys = $keys,
                signatures = $signatures,
                created_at = time::now(),
                updated_at = time::now()
        ";

        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("key_type", key_type.to_string()))
            .bind(("usage", serde_json::to_value(&key.usage)?))
            .bind(("keys", serde_json::to_value(&key.keys)?))
            .bind(("signatures", serde_json::to_value(&key.signatures)?))
            .await?;

        Ok(())
    }
}
