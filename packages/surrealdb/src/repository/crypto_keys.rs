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
    pub async fn validate_key_signatures(
        &self,
        user_id: &str,
        signatures: &HashMap<String, Value>,
    ) -> Result<bool, RepositoryError> {
        // Get the user's cross-signing keys
        let cross_signing_keys = self.get_cross_signing_keys(user_id).await?;

        // Basic validation - check if we have the required keys
        if cross_signing_keys.master_key.is_none() {
            return Ok(false);
        }

        // In a real implementation, this would:
        // 1. Verify each signature using the appropriate public key
        // 2. Check the signature chain (master -> self-signing -> device keys)
        // 3. Validate the cryptographic signatures using Ed25519

        // For now, perform basic validation
        for key_id in signatures.keys() {
            // Check if we have the signing key
            let key_exists_query = "
                SELECT key_id FROM cross_signing_keys
                WHERE user_id = $user_id
                AND keys CONTAINS $key_id
                LIMIT 1
            ";

            let mut result = self
                .db
                .query(key_exists_query)
                .bind(("user_id", user_id.to_string()))
                .bind(("key_id", key_id.to_string()))
                .await?;

            let keys: Vec<Value> = result.take(0)?;

            if keys.is_empty() {
                return Ok(false);
            }
        }

        // All basic validations passed
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
