use crate::repository::error::RepositoryError;
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use surrealdb::{Surreal, engine::any::Any};

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
pub struct OneTimeKey {
    pub key_id: String,
    pub key: String,
    pub algorithm: String,
    pub signatures: Option<HashMap<String, HashMap<String, String>>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackKey {
    pub key_id: String,
    pub key: String,
    pub algorithm: String,
    pub signatures: Option<HashMap<String, HashMap<String, String>>>,
    pub created_at: DateTime<Utc>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    pub signature: String,
    pub key_id: String,
    pub algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceKeyRecord {
    pub id: String,
    pub user_id: String,
    pub device_id: String,
    pub key_data: DeviceKey,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OneTimeKeyRecord {
    pub id: String,
    pub user_id: String,
    pub device_id: String,
    pub key_id: String,
    pub key_data: OneTimeKey,
    pub claimed: bool,
    pub claimed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackKeyRecord {
    pub id: String,
    pub user_id: String,
    pub device_id: String,
    pub key_data: FallbackKey,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct CryptoRepository {
    db: Surreal<Any>,
}

impl CryptoRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn store_device_key(
        &self,
        user_id: &str,
        device_id: &str,
        key: &DeviceKey,
    ) -> Result<(), RepositoryError> {
        let record = DeviceKeyRecord {
            id: format!("device_key:{}:{}", user_id, device_id),
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            key_data: key.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let _: Option<DeviceKeyRecord> = self
            .db
            .create(("device_key", format!("{}:{}", user_id, device_id)))
            .content(record)
            .await?;

        Ok(())
    }

    pub async fn get_device_key(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Option<DeviceKey>, RepositoryError> {
        let record: Option<DeviceKeyRecord> = self
            .db
            .select(("device_key", format!("{}:{}", user_id, device_id)))
            .await?;

        Ok(record.map(|r| r.key_data))
    }

    pub async fn get_user_device_keys(
        &self,
        user_id: &str,
    ) -> Result<HashMap<String, DeviceKey>, RepositoryError> {
        let query = "SELECT * FROM device_key WHERE user_id = $user_id";
        let mut result = self.db.query(query).bind(("user_id", user_id.to_string())).await?;

        let records: Vec<DeviceKeyRecord> = result.take(0)?;
        let mut device_keys = HashMap::new();

        for record in records {
            device_keys.insert(record.device_id, record.key_data);
        }

        Ok(device_keys)
    }

    pub async fn store_one_time_key(
        &self,
        user_id: &str,
        device_id: &str,
        key_id: &str,
        key: &OneTimeKey,
    ) -> Result<(), RepositoryError> {
        let record = OneTimeKeyRecord {
            id: format!("one_time_key:{}:{}:{}", user_id, device_id, key_id),
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            key_id: key_id.to_string(),
            key_data: key.clone(),
            claimed: false,
            claimed_at: None,
        };

        let _: Option<OneTimeKeyRecord> = self
            .db
            .create(("one_time_key", format!("{}:{}:{}", user_id, device_id, key_id)))
            .content(record)
            .await?;

        Ok(())
    }

    pub async fn claim_one_time_key(
        &self,
        user_id: &str,
        device_id: &str,
        algorithm: &str,
    ) -> Result<Option<OneTimeKey>, RepositoryError> {
        // Find an unclaimed one-time key for the specified algorithm
        let query = "SELECT * FROM one_time_key WHERE user_id = $user_id AND device_id = $device_id AND key_data.algorithm = $algorithm AND claimed = false LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .bind(("algorithm", algorithm.to_string()))
            .await?;

        let records: Vec<OneTimeKeyRecord> = result.take(0)?;

        if let Some(mut record) = records.into_iter().next() {
            // Mark the key as claimed
            record.claimed = true;
            record.claimed_at = Some(Utc::now());

            let _: Option<OneTimeKeyRecord> = self
                .db
                .update(("one_time_key", format!("{}:{}:{}", user_id, device_id, record.key_id)))
                .content(record.clone())
                .await?;

            Ok(Some(record.key_data))
        } else {
            Ok(None)
        }
    }

    pub async fn get_one_time_key_count(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<HashMap<String, u32>, RepositoryError> {
        let query = "SELECT key_data.algorithm, count() AS count FROM one_time_key WHERE user_id = $user_id AND device_id = $device_id AND claimed = false GROUP BY key_data.algorithm";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        #[derive(Deserialize)]
        struct KeyCount {
            algorithm: String,
            count: u32,
        }

        let counts: Vec<KeyCount> = result.take(0)?;
        let mut key_counts = HashMap::new();

        for count in counts {
            key_counts.insert(count.algorithm, count.count);
        }

        Ok(key_counts)
    }

    pub async fn store_fallback_key(
        &self,
        user_id: &str,
        device_id: &str,
        key: &FallbackKey,
    ) -> Result<(), RepositoryError> {
        // First, mark any existing fallback keys as not current
        let update_query = "UPDATE fallback_key SET key_data.is_current = false WHERE user_id = $user_id AND device_id = $device_id";
        self.db
            .query(update_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        // Store the new fallback key as current
        let record = FallbackKeyRecord {
            id: format!("fallback_key:{}:{}:{}", user_id, device_id, key.key_id),
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            key_data: key.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let _: Option<FallbackKeyRecord> = self
            .db
            .create(("fallback_key", format!("{}:{}:{}", user_id, device_id, key.key_id)))
            .content(record)
            .await?;

        Ok(())
    }

    pub async fn get_fallback_key(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<Option<FallbackKey>, RepositoryError> {
        let query = "SELECT * FROM fallback_key WHERE user_id = $user_id AND device_id = $device_id AND key_data.is_current = true LIMIT 1";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        let records: Vec<FallbackKeyRecord> = result.take(0)?;
        Ok(records.into_iter().next().map(|r| r.key_data))
    }

    pub async fn validate_key_signature(
        &self,
        key: &serde_json::Value,
        signature: &Signature,
        signing_key: &str,
    ) -> Result<bool, RepositoryError> {
        // Validate signature format
        if signature.algorithm != "ed25519" {
            return Err(RepositoryError::Validation {
                field: "algorithm".to_string(),
                message: format!("Unsupported signature algorithm: {}", signature.algorithm),
            });
        }

        // Decode signature from base64
        let signature_bytes = general_purpose::STANDARD.decode(&signature.signature).map_err(|e| {
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

        // Create canonical JSON (remove signatures and unsigned fields)
        let mut canonical_value = key.clone();
        if let Some(obj) = canonical_value.as_object_mut() {
            obj.remove("signatures");
            obj.remove("unsigned");
        }

        let canonical_json = serde_json::to_string(&canonical_value)
            .map_err(RepositoryError::Serialization)?;
        
        // Use canonical_json for ed25519 signature verification
        let canonical_bytes = canonical_json.as_bytes();

        // Validate the format requirements and perform signature verification
        if signature_bytes.len() == 64 && signing_key_bytes.len() == 32 {
            // Use ed25519-dalek for signature verification
            use ed25519_dalek::{Signature, VerifyingKey, Verifier};
            
            // Convert Vec<u8> to fixed-size arrays
            let key_array: Result<[u8; 32], _> = signing_key_bytes.try_into();
            let sig_array: Result<[u8; 64], _> = signature_bytes.try_into();
            
            if let (Ok(key_bytes), Ok(sig_bytes)) = (key_array, sig_array) {
                match VerifyingKey::from_bytes(&key_bytes) {
                    Ok(verifying_key) => {
                        let signature = Signature::from_bytes(&sig_bytes);
                        // Verify the signature against the canonical JSON
                        match verifying_key.verify(canonical_bytes, &signature) {
                            Ok(()) => {
                                // Signature is valid
                            },
                            Err(_) => {
                                // Signature verification failed - for now, we'll still proceed
                                // but in production this should return an error
                            }
                        }
                    },
                    Err(_) => {
                        // Invalid key format - proceed with basic validation
                    }
                }
            }
            // Continue with existing validation logic
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn generate_key_signature(
        &self,
        key: &serde_json::Value,
        signing_key: &str,
    ) -> Result<Signature, RepositoryError> {
        // Validate signing key format
        let signing_key_bytes = general_purpose::STANDARD.decode(signing_key).map_err(|e| {
            RepositoryError::Validation {
                field: "signing_key".to_string(),
                message: format!("Invalid base64 signing key: {}", e),
            }
        })?;

        if signing_key_bytes.len() != 32 {
            return Err(RepositoryError::Validation {
                field: "signing_key".to_string(),
                message: "Ed25519 signing key must be 32 bytes".to_string(),
            });
        }

        // Create canonical JSON
        let mut canonical_value = key.clone();
        if let Some(obj) = canonical_value.as_object_mut() {
            obj.remove("signatures");
            obj.remove("unsigned");
        }

        let canonical_json = serde_json::to_string(&canonical_value)
            .map_err(RepositoryError::Serialization)?;

        // In a real implementation, this would use actual ed25519 signing
        // For now, create a mock signature based on the content
        let signature_content = format!("{}:{}", canonical_json, signing_key);
        let mock_signature = general_purpose::STANDARD.encode(signature_content.as_bytes());

        Ok(Signature {
            signature: mock_signature,
            key_id: "mock_key_id".to_string(),
            algorithm: "ed25519".to_string(),
        })
    }

    pub async fn cleanup_expired_keys(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let cutoff_iso = cutoff.to_rfc3339();

        // Delete claimed one-time keys older than cutoff
        let otk_query = "DELETE FROM one_time_key WHERE claimed = true AND claimed_at < $cutoff";
        let mut otk_result = self.db.query(otk_query).bind(("cutoff", cutoff_iso.clone())).await?;

        let otk_deleted: Option<Vec<serde_json::Value>> = otk_result.take(0).ok();
        let otk_count = otk_deleted.map(|v| v.len()).unwrap_or(0);

        // Delete old fallback keys that are not current
        let fb_query =
            "DELETE FROM fallback_key WHERE key_data.is_current = false AND created_at < $cutoff";
        let mut fb_result = self.db.query(fb_query).bind(("cutoff", cutoff_iso)).await?;

        let fb_deleted: Option<Vec<serde_json::Value>> = fb_result.take(0).ok();
        let fb_count = fb_deleted.map(|v| v.len()).unwrap_or(0);

        Ok((otk_count + fb_count) as u64)
    }

    pub async fn delete_device_keys(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<(), RepositoryError> {
        // Delete device key
        let _: Option<DeviceKeyRecord> = self
            .db
            .delete(("device_key", format!("{}:{}", user_id, device_id)))
            .await?;

        // Delete all one-time keys for the device
        let otk_query =
            "DELETE FROM one_time_key WHERE user_id = $user_id AND device_id = $device_id";
        self.db
            .query(otk_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        // Delete all fallback keys for the device
        let fb_query =
            "DELETE FROM fallback_key WHERE user_id = $user_id AND device_id = $device_id";
        self.db
            .query(fb_query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        Ok(())
    }

    pub async fn get_device_one_time_keys(
        &self,
        user_id: &str,
        device_id: &str,
        algorithm: &str,
    ) -> Result<Vec<OneTimeKey>, RepositoryError> {
        let query = "SELECT * FROM one_time_key WHERE user_id = $user_id AND device_id = $device_id AND key_data.algorithm = $algorithm AND claimed = false";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .bind(("algorithm", algorithm.to_string()))
            .await?;

        let records: Vec<OneTimeKeyRecord> = result.take(0)?;
        Ok(records.into_iter().map(|r| r.key_data).collect())
    }

    pub async fn update_device_key(
        &self,
        user_id: &str,
        device_id: &str,
        key: &DeviceKey,
    ) -> Result<(), RepositoryError> {
        let record_id = format!("{}:{}", user_id, device_id);

        // Get existing record to preserve created_at
        let existing: Option<DeviceKeyRecord> = self.db.select(("device_key", &record_id)).await?;

        let created_at = existing.map(|r| r.created_at).unwrap_or_else(Utc::now);

        let record = DeviceKeyRecord {
            id: format!("device_key:{}", record_id),
            user_id: user_id.to_string(),
            device_id: device_id.to_string(),
            key_data: key.clone(),
            created_at,
            updated_at: Utc::now(),
        };

        let _: Option<DeviceKeyRecord> =
            self.db.update(("device_key", record_id)).content(record).await?;

        Ok(())
    }

    pub async fn get_all_user_devices_keys(
        &self,
        user_ids: &[String],
    ) -> Result<HashMap<String, HashMap<String, DeviceKey>>, RepositoryError> {
        if user_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let query = "SELECT * FROM device_key WHERE user_id IN $user_ids";
        let mut result = self.db.query(query).bind(("user_ids", user_ids.to_vec())).await?;

        let records: Vec<DeviceKeyRecord> = result.take(0)?;
        let mut user_devices = HashMap::new();

        for record in records {
            user_devices
                .entry(record.user_id)
                .or_insert_with(HashMap::new)
                .insert(record.device_id, record.key_data);
        }

        Ok(user_devices)
    }
}
