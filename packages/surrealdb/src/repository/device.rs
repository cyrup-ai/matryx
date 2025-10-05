use crate::repository::error::RepositoryError;
use futures_util::StreamExt;
use matryx_entity::types::{Device, DeviceKey, DeviceKeys};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};

/// Type alias for device query result tuple
type DeviceQueryResult = (String, Option<String>, Option<String>, Option<i64>, Option<String>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientDeviceInfo {
    pub device_id: String,
    pub display_name: Option<String>,
    pub last_seen_ip: Option<String>,
    pub last_seen_ts: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub device_keys: Option<serde_json::Value>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationDevice {
    pub device_id: String,
    pub user_id: String,
    pub device_keys: Option<DeviceKeys>,
    pub display_name: Option<String>,
    pub last_seen_ts: Option<i64>,
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceListResponse {
    pub devices: Vec<ClientDeviceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceKeysResponse {
    pub device_keys: HashMap<String, HashMap<String, DeviceKeys>>,
    pub failures: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneTimeKey {
    pub key_id: String,
    pub key: String,
    pub algorithm: String,
    pub signatures: Option<HashMap<String, HashMap<String, String>>>,
}

#[derive(Clone)]
pub struct DeviceRepository {
    db: Surreal<Any>,
}

impl DeviceRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create(&self, device: &Device) -> Result<Device, RepositoryError> {
        let device_clone = device.clone();
        let created: Option<Device> =
            self.db.create(("device", &device.device_id)).content(device_clone).await?;
        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create device"))
        })
    }

    pub async fn get_by_id(&self, device_id: &str) -> Result<Option<Device>, RepositoryError> {
        let device: Option<Device> = self.db.select(("device", device_id)).await?;
        Ok(device)
    }

    pub async fn update(&self, device: &Device) -> Result<Device, RepositoryError> {
        let device_clone = device.clone();
        let updated: Option<Device> =
            self.db.update(("device", &device.device_id)).content(device_clone).await?;
        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update device"))
        })
    }

    pub async fn delete(&self, device_id: &str) -> Result<(), RepositoryError> {
        let _: Option<Device> = self.db.delete(("device", device_id)).await?;
        Ok(())
    }

    pub async fn get_user_devices(&self, user_id: &str) -> Result<Vec<Device>, RepositoryError> {
        let query = "SELECT * FROM device WHERE user_id = $user_id";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let devices: Vec<Device> = result.take(0)?;
        Ok(devices)
    }

    pub async fn delete_user_devices(&self, user_id: &str) -> Result<(), RepositoryError> {
        let query = "DELETE FROM device WHERE user_id = $user_id";
        self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        Ok(())
    }

    pub async fn get_by_user(&self, user_id: &str) -> Result<Vec<Device>, RepositoryError> {
        self.get_user_devices(user_id).await
    }

    pub async fn get_by_user_and_device(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Option<Device>, RepositoryError> {
        let query =
            "SELECT * FROM device WHERE user_id = $user_id AND device_id = $device_id LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;
        let devices: Vec<Device> = result.take(0)?;
        Ok(devices.into_iter().next())
    }

    /// Enhanced device creation with comprehensive metadata
    pub async fn create_device_with_metadata(
        &self,
        device_info: Device,
        initial_keys: Option<DeviceKey>,
    ) -> Result<Device, RepositoryError> {
        let mut query = String::from("CREATE device SET");
        query.push_str(" device_id = $device_id,");
        query.push_str(" user_id = $user_id,");
        query.push_str(" display_name = $display_name,");
        query.push_str(" created_at = time::now(),");
        query.push_str(" last_seen_ts = time::now(),");
        query.push_str(" last_seen_ip = $last_seen_ip");

        if let Some(_keys) = &initial_keys {
            query.push_str(", device_keys = $device_keys");
        }

        let device_id = device_info.device_id.clone();
        let user_id = device_info.user_id.clone();
        let display_name = device_info.display_name.clone();
        let last_seen_ip = device_info.last_seen_ip.clone();

        let mut query_builder = self
            .db
            .query(query)
            .bind(("device_id", device_id))
            .bind(("user_id", user_id))
            .bind(("display_name", display_name))
            .bind(("last_seen_ip", last_seen_ip));

        if let Some(keys) = initial_keys {
            query_builder = query_builder.bind(("device_keys", serde_json::to_value(keys)?));
        }

        let mut result = query_builder.await?;

        let created_device: Option<Device> = result.take(0)?;
        created_device.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create device"))
        })
    }

    /// Update device activity with real-time tracking
    pub async fn update_device_activity(
        &self,
        device_id: &str,
        user_id: &str,
        ip_address: Option<String>,
    ) -> Result<(), RepositoryError> {
        let device_id = device_id.to_string();
        let user_id = user_id.to_string();

        self.db
            .query("UPDATE device SET last_seen_ts = time::now(), last_seen_ip = $ip WHERE device_id = $device_id AND user_id = $user_id")
            .bind(("device_id", device_id))
            .bind(("user_id", user_id))
            .bind(("ip", ip_address))
            .await?;

        Ok(())
    }

    /// Bulk device operations for federation efficiency
    pub async fn get_devices_for_users(
        &self,
        user_ids: Vec<String>,
    ) -> Result<HashMap<String, Vec<Device>>, RepositoryError> {
        let mut result = HashMap::new();

        let query = "SELECT * FROM device WHERE user_id IN $user_ids";
        let mut response = self.db.query(query).bind(("user_ids", user_ids.clone())).await?;

        let devices: Vec<Device> = response.take(0)?;

        for device in devices {
            result.entry(device.user_id.clone()).or_insert_with(Vec::new).push(device);
        }

        Ok(result)
    }

    /// Get all devices for a user
    pub async fn get_all_user_devices(
        &self,
        user_id: &str,
    ) -> Result<Vec<Device>, RepositoryError> {
        self.get_user_devices(user_id).await
    }

    /// Verify if a device exists and belongs to the user
    pub async fn verify_device(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "SELECT count() FROM device WHERE user_id = $user_id AND device_id = $device_id GROUP ALL";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    /// Check if a device exists by device ID
    pub async fn device_exists(&self, device_id: &str) -> Result<bool, RepositoryError> {
        let query = "SELECT count() FROM device WHERE device_id = $device_id GROUP ALL";
        let mut result = self.db.query(query).bind(("device_id", device_id.to_string())).await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    /// Subscribe to device key changes for a specific user using SurrealDB LiveQuery
    /// Returns a stream of notifications for device key changes for the specified user
    pub async fn subscribe_to_device_keys(
        &self,
        user_id: &str,
    ) -> Result<impl futures_util::Stream<Item = Result<Device, RepositoryError>>, RepositoryError>
    {
        // Create SurrealDB LiveQuery for device keys for specific user
        let mut stream = self
            .db
            .query("LIVE SELECT * FROM device WHERE user_id = $user_id")
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        // Transform SurrealDB notification stream to device stream
        let device_stream = stream
            .stream::<surrealdb::Notification<Device>>(0)
            .map_err(RepositoryError::Database)?
            .map(|notification_result| -> Result<Device, RepositoryError> {
                let notification = notification_result.map_err(RepositoryError::Database)?;

                match notification.action {
                    surrealdb::Action::Create | surrealdb::Action::Update => Ok(notification.data),
                    surrealdb::Action::Delete => {
                        // For deleted devices, return the device data for proper handling
                        Ok(notification.data)
                    },
                    _ => {
                        // Handle any future Action variants
                        Ok(notification.data)
                    },
                }
            });

        Ok(device_stream)
    }

    // Device management methods for infrastructure

    /// Get list of user devices formatted for client API
    pub async fn get_user_devices_list(
        &self,
        user_id: &str,
    ) -> Result<Vec<ClientDeviceInfo>, RepositoryError> {
        let query = "SELECT device_id, display_name, last_seen_ip, last_seen_ts FROM device WHERE user_id = $user_id";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let devices: Vec<ClientDeviceInfo> = result.take(0)?;
        Ok(devices)
    }

    /// Get specific device info for client API
    pub async fn get_device_info(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Option<ClientDeviceInfo>, RepositoryError> {
        let query = "SELECT device_id, display_name, last_seen_ip, last_seen_ts FROM device WHERE user_id = $user_id AND device_id = $device_id LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;
        let devices: Vec<ClientDeviceInfo> = result.take(0)?;
        Ok(devices.into_iter().next())
    }

    /// Update device display name
    pub async fn update_device_info(
        &self,
        user_id: &str,
        device_id: &str,
        display_name: Option<String>,
    ) -> Result<(), RepositoryError> {
        let query = "UPDATE device SET display_name = $display_name WHERE user_id = $user_id AND device_id = $device_id";
        let mut _result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .bind(("display_name", display_name))
            .await?;
        Ok(())
    }

    /// Delete a specific device
    pub async fn delete_device(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<(), RepositoryError> {
        let query = "DELETE FROM device WHERE user_id = $user_id AND device_id = $device_id";
        let mut _result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;
        Ok(())
    }

    /// Delete multiple user devices by device IDs
    pub async fn delete_user_devices_by_ids(
        &self,
        user_id: &str,
        device_ids: &[String],
    ) -> Result<u32, RepositoryError> {
        let query = "DELETE FROM device WHERE user_id = $user_id AND device_id IN $device_ids RETURN BEFORE";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_ids", device_ids.to_vec()))
            .await?;
        let deleted: Vec<Device> = result.take(0)?;
        Ok(deleted.len() as u32)
    }

    /// Validate that a device belongs to a user
    pub async fn validate_device_ownership(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<bool, RepositoryError> {
        let query =
            "SELECT VALUE count() FROM device WHERE user_id = $user_id AND device_id = $device_id";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;
        let count: Option<i64> = result.take(0)?;
        Ok(count.unwrap_or(0) > 0)
    }

    /// Get access tokens associated with a device
    pub async fn get_device_access_tokens(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let query = "SELECT VALUE token FROM access_token WHERE user_id = $user_id AND device_id = $device_id";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;
        let tokens: Vec<String> = result.take(0)?;
        Ok(tokens)
    }

    // Federation-specific device methods

    /// Get user devices formatted for federation
    pub async fn get_user_devices_for_federation(
        &self,
        user_id: &str,
    ) -> Result<Vec<FederationDevice>, RepositoryError> {
        let query = "
            SELECT 
                device_id,
                user_id,
                display_name,
                last_seen_ts,
                device_keys
            FROM device 
            WHERE user_id = $user_id AND is_active = true
        ";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let devices_data: Vec<serde_json::Value> = result.take(0)?;

        let mut devices = Vec::new();
        for device_data in devices_data {
            let device = FederationDevice {
                device_id: device_data
                    .get("device_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                user_id: device_data
                    .get("user_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                device_keys: device_data
                    .get("device_keys")
                    .and_then(|v| serde_json::from_value(v.clone()).ok()),
                display_name: device_data
                    .get("display_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                last_seen_ts: device_data.get("last_seen_ts").and_then(|v| v.as_i64()),
            };
            devices.push(device);
        }

        Ok(devices)
    }

    /// Get device keys for federation query
    pub async fn get_device_keys_for_federation(
        &self,
        user_id: &str,
        device_ids: &[String],
    ) -> Result<DeviceKeysResponse, RepositoryError> {
        let mut device_keys = HashMap::new();
        let failures = HashMap::new();

        let query = "
            SELECT device_id, device_keys FROM device 
            WHERE user_id = $user_id AND device_id IN $device_ids
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_ids", device_ids.to_vec()))
            .await?;
        let devices_data: Vec<serde_json::Value> = result.take(0)?;

        let mut user_devices = HashMap::new();
        for device_data in devices_data {
            if let (Some(device_id), Some(keys_value)) = (
                device_data.get("device_id").and_then(|v| v.as_str()),
                device_data.get("device_keys"),
            )
                && let Ok(keys) = serde_json::from_value::<DeviceKeys>(keys_value.clone()) {
                    user_devices.insert(device_id.to_string(), keys);
                }
        }

        if !user_devices.is_empty() {
            device_keys.insert(user_id.to_string(), user_devices);
        }

        Ok(DeviceKeysResponse { device_keys, failures })
    }

    /// Claim one-time keys for federation
    pub async fn claim_one_time_keys(
        &self,
        user_id: &str,
        device_id: &str,
        algorithm: &str,
    ) -> Result<Option<OneTimeKey>, RepositoryError> {
        // Get and remove one available one-time key
        let query = "
            SELECT * FROM one_time_keys 
            WHERE user_id = $user_id AND device_id = $device_id AND algorithm = $algorithm
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .bind(("algorithm", algorithm.to_string()))
            .await?;
        let keys_data: Vec<serde_json::Value> = result.take(0)?;

        if let Some(key_data) = keys_data.first() {
            let key = OneTimeKey {
                key_id: key_data.get("key_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                key: key_data.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                algorithm: algorithm.to_string(),
                signatures: key_data
                    .get("signatures")
                    .and_then(|v| serde_json::from_value(v.clone()).ok()),
            };

            // Remove the claimed key
            let delete_query = "
                DELETE one_time_keys 
                WHERE user_id = $user_id AND device_id = $device_id AND key_id = $key_id
            ";
            self.db
                .query(delete_query)
                .bind(("user_id", user_id.to_string()))
                .bind(("device_id", device_id.to_string()))
                .bind(("key_id", key.key_id.clone()))
                .await?;

            return Ok(Some(key));
        }

        Ok(None)
    }

    /// Upload device keys for federation
    pub async fn upload_device_keys(
        &self,
        user_id: &str,
        device_keys: &DeviceKeys,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE device SET device_keys = $device_keys 
            WHERE user_id = $user_id AND device_id = $device_id
        ";
        self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_keys.device_id.clone()))
            .bind(("device_keys", serde_json::to_value(device_keys)?))
            .await?;

        Ok(())
    }

    /// Query device keys for multiple users and devices
    pub async fn query_device_keys(
        &self,
        user_devices: &[(String, Vec<String>)],
    ) -> Result<DeviceKeysResponse, RepositoryError> {
        let mut device_keys = HashMap::new();
        let mut failures = HashMap::new();

        for (user_id, device_ids) in user_devices {
            match self.get_device_keys_for_federation(user_id, device_ids).await {
                Ok(response) => {
                    for (uid, devices) in response.device_keys {
                        device_keys.insert(uid, devices);
                    }
                    for (uid, failure) in response.failures {
                        failures.insert(uid, failure);
                    }
                },
                Err(_) => {
                    failures.insert(
                        user_id.clone(),
                        serde_json::json!({
                            "error": "Failed to query device keys"
                        }),
                    );
                },
            }
        }

        Ok(DeviceKeysResponse { device_keys, failures })
    }

    /// Upload one-time keys for a device
    pub async fn upload_one_time_keys(
        &self,
        user_id: &str,
        device_id: &str,
        keys: &HashMap<String, serde_json::Value>,
    ) -> Result<(), RepositoryError> {
        for (key_id, key_data) in keys {
            let query = "
                CREATE one_time_keys SET
                user_id = $user_id,
                device_id = $device_id,
                key_id = $key_id,
                algorithm = $algorithm,
                key = $key,
                signatures = $signatures,
                created_at = $created_at
            ";

            // Extract algorithm from key_id (format: algorithm:key_id)
            let algorithm = if let Some(colon_pos) = key_id.find(':') {
                &key_id[..colon_pos]
            } else {
                "signed_curve25519" // Default algorithm
            };

            self.db
                .query(query)
                .bind(("user_id", user_id.to_string()))
                .bind(("device_id", device_id.to_string()))
                .bind(("key_id", key_id.clone()))
                .bind(("algorithm", algorithm.to_string()))
                .bind(("key", key_data.clone()))
                .bind(("signatures", serde_json::Value::Null)) // Would extract from key_data in real implementation
                .bind(("created_at", chrono::Utc::now()))
                .await?;
        }

        Ok(())
    }

    /// Get one-time key counts for a device
    pub async fn get_one_time_key_counts(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<HashMap<String, i64>, RepositoryError> {
        let query = "
            SELECT algorithm, count() as count FROM one_time_keys 
            WHERE user_id = $user_id AND device_id = $device_id
            GROUP BY algorithm
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;
        let counts_data: Vec<serde_json::Value> = result.take(0)?;

        let mut counts = HashMap::new();
        for count_data in counts_data {
            if let (Some(algorithm), Some(count)) = (
                count_data.get("algorithm").and_then(|v| v.as_str()),
                count_data.get("count").and_then(|v| v.as_i64()),
            ) {
                counts.insert(algorithm.to_string(), count);
            }
        }

        // Ensure we always have a count for signed_curve25519 (Matrix requirement)
        if !counts.contains_key("signed_curve25519") {
            counts.insert("signed_curve25519".to_string(), 0);
        }

        Ok(counts)
    }

    /// Get user device keys for federation batch query
    pub async fn get_user_device_keys_for_federation_batch(&self, user_id: &str) -> Result<HashMap<String, DeviceKeys>, RepositoryError> {
        let query = "
            SELECT device_id, device_keys FROM device 
            WHERE user_id = $user_id AND device_keys IS NOT NULL
        ";
        
        let mut result = self.db.query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;
        let devices_data: Vec<serde_json::Value> = result.take(0)?;

        let mut device_keys = HashMap::new();
        for device_data in devices_data {
            if let (Some(device_id), Some(keys_value)) = (
                device_data.get("device_id").and_then(|v| v.as_str()),
                device_data.get("device_keys"),
            )
                && let Ok(keys) = serde_json::from_value::<DeviceKeys>(keys_value.clone()) {
                    device_keys.insert(device_id.to_string(), keys);
                }
        }

        Ok(device_keys)
    }

    /// Validate device key query parameters
    pub async fn validate_device_key_query(&self, user_id: &str, device_ids: &[String]) -> Result<bool, RepositoryError> {
        if device_ids.is_empty() {
            return Ok(true); // Empty device list means query all devices
        }

        let query = "
            SELECT COUNT() as count FROM device 
            WHERE user_id = $user_id AND device_id IN $device_ids
        ";
        
        let mut result = self.db.query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_ids", device_ids.to_vec()))
            .await?;
        let counts: Vec<serde_json::Value> = result.take(0)?;

        if let Some(count_obj) = counts.first()
            && let Some(count) = count_obj.get("count").and_then(|v| v.as_u64()) {
                return Ok(count > 0);
            }
        
        Ok(false)
    }

    /// Create device info for login flows (SSO, AS)
    pub async fn create_device_info(
        &self,
        user_id: &str,
        device_id: &str,
        display_name: Option<String>,
        client_ip: &str,
        user_agent: Option<String>,
        application_service_id: Option<String>,
    ) -> Result<(), RepositoryError> {
        let device_info = serde_json::json!({
            "device_id": device_id,
            "user_id": user_id,
            "display_name": display_name,
            "created_at": chrono::Utc::now(),
            "last_seen_ip": client_ip,
            "last_seen_user_agent": user_agent.unwrap_or_else(|| "unknown".to_string()),
            "application_service_id": application_service_id
        });

        let _: Option<serde_json::Value> = self.db
            .create(("devices", format!("{}:{}", user_id, device_id)))
            .content(device_info)
            .await?;

        Ok(())
    }

    /// Get user devices for admin whois
    pub async fn get_user_devices_for_admin(
        &self,
        user_id: &str,
    ) -> Result<Vec<DeviceQueryResult>, RepositoryError> {
        let query = "
            SELECT d.device_id, d.display_name, d.last_seen_ip, d.last_seen_ts, d.user_agent
            FROM device d
            WHERE d.user_id = $user_id
        ";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;
        let devices: Vec<DeviceQueryResult> = result.take(0)?;
        Ok(devices)
    }

    /// Create device for device management endpoints
    pub async fn create_device(
        &self,
        user_id: &str,
        display_name: Option<&str>,
        last_seen_ip: Option<&str>,
        device_keys: Option<serde_json::Value>,
    ) -> Result<Device, RepositoryError> {
        use uuid::Uuid;
        use chrono::Utc;
        use matryx_entity::types::Device;

        let device_id = Uuid::new_v4().simple().to_string().to_uppercase();
        let now = Utc::now();

        let device = Device {
            device_id: device_id.clone(),
            user_id: user_id.to_string(),
            display_name: display_name.map(|s| s.to_string()),
            last_seen_ip: last_seen_ip.map(|s| s.to_string()),
            last_seen_ts: Some(now.timestamp_millis()),
            created_at: now,
            hidden: Some(false),
            device_keys,
            one_time_keys: None,
            fallback_keys: None,
            user_agent: None,
            initial_device_display_name: display_name.map(|s| s.to_string()),
        };

        self.create(&device).await
    }

    /// Get pending to-device events for a specific device
    pub async fn get_pending_to_device_events(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<serde_json::Value, crate::repository::error::RepositoryError> {
        // Query to-device events table for pending events
        let events: Vec<serde_json::Value> = self.db
            .query("SELECT * FROM to_device_events WHERE user_id = $user_id AND device_id = $device_id AND delivered = false ORDER BY created_at ASC")
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?
            .take(0)?;

        // Mark events as delivered after fetching
        let _: Vec<serde_json::Value> = self.db
            .query("UPDATE to_device_events SET delivered = true WHERE user_id = $user_id AND device_id = $device_id AND delivered = false")
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?
            .take(0)?;

        Ok(serde_json::json!({
            "events": events
        }))
    }

    /// Get to-device events since a specific timestamp
    pub async fn get_to_device_events_since(
        &self,
        user_id: &str,
        device_id: &str,
        since_timestamp: i64,
    ) -> Result<serde_json::Value, crate::repository::error::RepositoryError> {
        // Query to-device events since timestamp
        let events: Vec<serde_json::Value> = self.db
            .query("SELECT * FROM to_device_events WHERE user_id = $user_id AND device_id = $device_id AND created_at > $since ORDER BY created_at ASC")
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .bind(("since", since_timestamp))
            .await?
            .take(0)?;

        // Mark events as delivered after fetching
        let _: Vec<serde_json::Value> = self.db
            .query("UPDATE to_device_events SET delivered = true WHERE user_id = $user_id AND device_id = $device_id AND created_at > $since")
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .bind(("since", since_timestamp))
            .await?
            .take(0)?;

        Ok(serde_json::json!({
            "events": events
        }))
    }

    /// Get device list changes since a specific timestamp
    pub async fn get_device_list_changes_since(
        &self,
        user_id: &str,
        since_timestamp: i64,
    ) -> Result<serde_json::Value, crate::repository::error::RepositoryError> {
        // Query device list changes since timestamp
        let changed_devices: Vec<serde_json::Value> = self.db
            .query("SELECT user_id, device_id FROM device_list_updates WHERE user_id = $user_id AND updated_at > $since ORDER BY updated_at ASC")
            .bind(("user_id", user_id.to_string()))
            .bind(("since", since_timestamp))
            .await?
            .take(0)?;

        // Get list of users whose devices have changed
        let left_devices: Vec<serde_json::Value> = self.db
            .query("SELECT user_id, device_id FROM device_list_updates WHERE user_id = $user_id AND updated_at > $since AND action = 'left' ORDER BY updated_at ASC")
            .bind(("user_id", user_id.to_string()))
            .bind(("since", since_timestamp))
            .await?
            .take(0)?;

        Ok(serde_json::json!({
            "changed": changed_devices,
            "left": left_devices
        }))
    }
}

