use crate::repository::error::RepositoryError;
use matryx_entity::DeviceKeys;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserKeys {
    pub device_keys: HashMap<String, DeviceKeys>,
    pub master_key: Option<Value>,
    pub self_signing_key: Option<Value>,
    pub user_signing_key: Option<Value>,
}

#[derive(Clone)]
pub struct KeysRepository {
    db: Surreal<Any>,
}

impl KeysRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    /// Store device keys for a user's device
    pub async fn store_device_keys(
        &self,
        user_id: &str,
        device_id: &str,
        device_keys: &DeviceKeys,
    ) -> Result<(), RepositoryError> {
        let device_keys_data = serde_json::json!({
            "user_id": user_id,
            "device_id": device_id,
            "device_keys": device_keys,
            "created_at": Utc::now(),
            "signature_valid": true,
            "validation_timestamp": Utc::now()
        });

        let _: Option<Value> = self.db
            .create(("device_keys", format!("{}:{}", user_id, device_id)))
            .content(device_keys_data)
            .await?;

        Ok(())
    }

    /// Store one-time keys for a device
    pub async fn store_one_time_keys(
        &self,
        user_id: &str,
        device_id: &str,
        one_time_keys: &HashMap<String, Value>,
    ) -> Result<(), RepositoryError> {
        for (key_id, key_data) in one_time_keys {
            let key_data_json = serde_json::json!({
                "key_id": key_id,
                "key": key_data,
                "user_id": user_id,
                "device_id": device_id,
                "created_at": Utc::now(),
                "claimed": false,
                "algorithm_type": key_id.split(':').next().unwrap_or("unknown"),
                "vodozemac_validated": true
            });

            let _: Option<Value> = self.db
                .create(("one_time_keys", format!("{}:{}:{}", user_id, device_id, key_id)))
                .content(key_data_json)
                .await?;
        }

        Ok(())
    }

    /// Store fallback keys for a device
    pub async fn store_fallback_keys(
        &self,
        user_id: &str,
        device_id: &str,
        fallback_keys: &HashMap<String, Value>,
    ) -> Result<(), RepositoryError> {
        for (key_id, key_data) in fallback_keys {
            let fallback_data = serde_json::json!({
                "key_id": key_id,
                "key": key_data,
                "user_id": user_id,
                "device_id": device_id,
                "created_at": Utc::now()
            });

            let _: Option<Value> = self.db
                .create(("fallback_keys", format!("{}:{}:{}", user_id, device_id, key_id)))
                .content(fallback_data)
                .await?;
        }

        Ok(())
    }

    /// Get one-time key counts for a device
    pub async fn get_one_time_key_counts(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<HashMap<String, u32>, RepositoryError> {
        let query = "SELECT algorithm_type, count() AS count FROM one_time_keys WHERE user_id = $user_id AND device_id = $device_id AND claimed = false GROUP BY algorithm_type";
        
        let mut response = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        let results: Vec<Value> = response.take(0)?;

        let mut counts = HashMap::new();
        for result in results {
            if let (Some(algorithm), Some(count)) = (
                result.get("algorithm_type").and_then(|v| v.as_str()),
                result.get("count").and_then(|v| v.as_u64()),
            ) {
                counts.insert(algorithm.to_string(), count as u32);
            }
        }

        Ok(counts)
    }

    /// Query local user keys (all devices)
    pub async fn query_local_user_keys_all_devices(
        &self,
        user_id: &str,
    ) -> Result<UserKeys, RepositoryError> {
        let query = "SELECT * FROM device_keys WHERE user_id = $user_id";
        
        let mut response = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        let devices: Vec<Value> = response.take(0)?;

        let mut user_device_keys: HashMap<String, DeviceKeys> = HashMap::new();
        for device in devices {
            #[allow(clippy::collapsible_if)]
            if let Some(device_data) = device.get("device_keys") {
                if let Ok(device_keys) = serde_json::from_value::<DeviceKeys>(device_data.clone()) {
                    user_device_keys.insert(device_keys.device_id.clone(), device_keys);
                }
            }
        }

        // Query cross-signing keys for this user
        let (master_key, self_signing_key, user_signing_key) =
            self.query_cross_signing_keys(user_id).await?;

        Ok(UserKeys {
            device_keys: user_device_keys,
            master_key,
            self_signing_key,
            user_signing_key,
        })
    }

    /// Query local user keys (specific devices)
    pub async fn query_local_user_keys_specific_devices(
        &self,
        user_id: &str,
        device_ids: &[String],
    ) -> Result<UserKeys, RepositoryError> {
        let mut user_device_keys: HashMap<String, DeviceKeys> = HashMap::new();

        // Query specific devices
        for device_id in device_ids {
            let query = "SELECT * FROM device_keys WHERE user_id = $user_id AND device_id = $device_id";
            
            let mut response = self.db
                .query(query)
                .bind(("user_id", user_id.to_string()))
                .bind(("device_id", device_id.clone()))
                .await?;

            let device: Option<Value> = response.take(0)?;

            #[allow(clippy::collapsible_if)]
            if let Some(device_data) = device
                && let Some(keys) = device_data.get("device_keys")
                && let Ok(device_keys) = serde_json::from_value::<DeviceKeys>(keys.clone()) {
                user_device_keys.insert(device_id.clone(), device_keys);
            }
        }

        // Query cross-signing keys for this user
        let (master_key, self_signing_key, user_signing_key) =
            self.query_cross_signing_keys(user_id).await?;

        Ok(UserKeys {
            device_keys: user_device_keys,
            master_key,
            self_signing_key,
            user_signing_key,
        })
    }

    /// Query cross-signing keys for a user
    pub async fn query_cross_signing_keys(
        &self,
        user_id: &str,
    ) -> Result<(Option<Value>, Option<Value>, Option<Value>), RepositoryError> {
        let query = "SELECT * FROM cross_signing_keys WHERE user_id = $user_id";
        
        let mut response = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        let cross_signing_keys: Vec<Value> = response.take(0)?;

        let mut master_key = None;
        let mut self_signing_key = None;
        let mut user_signing_key = None;

        for key in cross_signing_keys {
            #[allow(clippy::collapsible_if)]
            if let Some(key_type) = key.get("key_type").and_then(|v| v.as_str())
                && let Some(key_data) = key.get("key_data") {
                match key_type {
                    "master" => master_key = Some(key_data.clone()),
                    "self_signing" => self_signing_key = Some(key_data.clone()),
                    "user_signing" => user_signing_key = Some(key_data.clone()),
                    _ => {},
                }
            }
        }

        Ok((master_key, self_signing_key, user_signing_key))
    }

    /// Check if users share any rooms (for key access authorization)
    pub async fn can_access_user_keys(
        &self,
        requesting_user_id: &str,
        target_user_id: &str,
    ) -> Result<bool, RepositoryError> {
        // Users can always access their own keys
        if requesting_user_id == target_user_id {
            return Ok(true);
        }

        // Check if users share any rooms by querying room membership
        let query = r#"
            SELECT room_id FROM room_memberships
            WHERE user_id = $requesting_user_id AND membership = 'join'
            INTERSECT
            SELECT room_id FROM room_memberships
            WHERE user_id = $target_user_id AND membership = 'join'
            LIMIT 1
        "#;

        let mut response = self.db
            .query(query)
            .bind(("requesting_user_id", requesting_user_id.to_string()))
            .bind(("target_user_id", target_user_id.to_string()))
            .await?;

        let rooms: Vec<Value> = response.take(0)?;
        Ok(!rooms.is_empty())
    }

    /// Find and claim one-time keys
    pub async fn claim_one_time_keys(
        &self,
        user_id: &str,
        device_id: &str,
        algorithm: &str,
    ) -> Result<Option<(String, Value)>, RepositoryError> {
        // Find an available one-time key for this user/device/algorithm
        let query = "
            SELECT * FROM one_time_keys 
            WHERE user_id = $user_id 
              AND device_id = $device_id 
              AND key_id LIKE $algorithm_pattern
              AND claimed = false 
            LIMIT 1
        ";

        let algorithm_pattern = format!("{}:%", algorithm);
        let mut response = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .bind(("algorithm_pattern", algorithm_pattern))
            .await?;

        let key_record: Option<Value> = response.take(0)?;

        if let Some(key_data) = key_record {
            let key_id = key_data.get("key_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let key_value = key_data.get("key").cloned().unwrap_or(serde_json::json!({}));

            // Mark the key as claimed
            let update_query = "UPDATE one_time_keys SET claimed = true WHERE user_id = $user_id AND device_id = $device_id AND key_id = $key_id";
            self.db
                .query(update_query)
                .bind(("user_id", user_id.to_string()))
                .bind(("device_id", device_id.to_string()))
                .bind(("key_id", key_id.clone()))
                .await?;

            return Ok(Some((key_id, key_value)));
        }

        Ok(None)
    }

    /// Find fallback keys
    pub async fn find_fallback_keys(
        &self,
        user_id: &str,
        device_id: &str,
        algorithm: &str,
    ) -> Result<Option<(String, Value)>, RepositoryError> {
        let fallback_query = "
            SELECT * FROM fallback_keys 
            WHERE user_id = $user_id 
              AND device_id = $device_id 
              AND key_id LIKE $algorithm_pattern
            LIMIT 1
        ";

        let algorithm_pattern = format!("{}:%", algorithm);
        let mut fallback_response = self.db
            .query(fallback_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .bind(("algorithm_pattern", algorithm_pattern))
            .await?;

        let fallback_key: Option<Value> = fallback_response.take(0)?;

        if let Some(fallback_data) = fallback_key {
            let key_id = fallback_data
                .get("key_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let key_value = fallback_data.get("key").cloned().unwrap_or(serde_json::json!({}));

            return Ok(Some((key_id, key_value)));
        }

        Ok(None)
    }

    /// Update device key signatures
    pub async fn update_device_key_signatures(
        &self,
        target_user_id: &str,
        device_id: &str,
        signing_user_id: &str,
        signatures: &HashMap<String, String>,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE device_keys 
            SET signatures = array::union(signatures, $new_signatures), updated_at = $updated_at
            WHERE user_id = $user_id AND device_id = $device_id
        ";

        self.db
            .query(query)
            .bind(("user_id", target_user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .bind(("new_signatures", serde_json::json!({ signing_user_id: signatures })))
            .bind(("updated_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Update cross-signing key signatures
    pub async fn update_cross_signing_key_signatures(
        &self,
        target_user_id: &str,
        key_type: &str,
        signing_user_id: &str,
        signatures: &HashMap<String, String>,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE cross_signing_keys 
            SET signatures = array::union(signatures, $new_signatures), created_at = $updated_at
            WHERE user_id = $user_id AND key_type = $key_type
        ";

        self.db
            .query(query)
            .bind(("user_id", target_user_id.to_string()))
            .bind(("key_type", key_type.to_string()))
            .bind(("new_signatures", serde_json::json!({ signing_user_id: signatures })))
            .bind(("updated_at", Utc::now()))
            .await?;

        Ok(())
    }
}