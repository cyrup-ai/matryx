use chrono::{DateTime, Utc};
use matryx_entity::types::{Device, DeviceKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

use matryx_surrealdb::repository::{DeviceRepository, EDURepository, RepositoryError};

#[derive(Debug, Clone)]
pub enum DeviceError {
    DatabaseError(String),
    MissingPreviousUpdate,
    InvalidUpdate(String),
    NetworkError(String),
}

impl std::fmt::Display for DeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            DeviceError::MissingPreviousUpdate => write!(f, "Missing previous update in chain"),
            DeviceError::InvalidUpdate(msg) => write!(f, "Invalid update: {}", msg),
            DeviceError::NetworkError(msg) => write!(f, "Network error: {}", msg),
        }
    }
}

impl std::error::Error for DeviceError {}

impl From<serde_json::Error> for DeviceError {
    fn from(err: serde_json::Error) -> Self {
        DeviceError::InvalidUpdate(format!("JSON serialization error: {}", err))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceListUpdate {
    pub user_id: String,
    pub device_id: String,
    pub stream_id: i64,
    pub prev_id: Vec<i64>,
    pub deleted: bool,
    pub device_display_name: Option<String>,
    pub keys: Option<DeviceKey>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CrossSigningKey {
    pub user_id: String,
    pub usage: Vec<String>,
    pub keys: HashMap<String, String>,
    pub signatures: Option<HashMap<String, HashMap<String, String>>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceListCache {
    pub devices: Vec<Device>,
    pub stream_id: i64,
    pub cached_at: chrono::DateTime<chrono::Utc>,
}

impl DeviceListCache {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            stream_id: 0,
            cached_at: chrono::Utc::now(),
        }
    }
}

/// Device manager using repository pattern for persistent storage
pub struct DeviceManager {
    device_repo: Arc<DeviceRepository>,
    edu_repo: Arc<EDURepository<surrealdb::engine::any::Any>>,
}

impl DeviceManager {
    pub fn new(
        device_repo: Arc<DeviceRepository>,
        edu_repo: Arc<EDURepository<surrealdb::engine::any::Any>>,
    ) -> Self {
        Self { device_repo, edu_repo }
    }

    /// Apply incremental device list updates with dependency validation
    pub async fn apply_device_update(&self, update: &DeviceListUpdate) -> Result<(), DeviceError> {
        // Validate update sequence using EDU repository
        let can_apply = self.can_apply_update(update).await?;
        if !can_apply {
            return Err(DeviceError::MissingPreviousUpdate);
        }

        if update.deleted {
            self.device_repo.delete(&update.device_id).await.map_err(|e| {
                DeviceError::DatabaseError(format!("Failed to delete device: {:?}", e))
            })?;
            info!("Device {} deleted for user {}", update.device_id, update.user_id);
        } else {
            let device_info = Device {
                device_id: update.device_id.clone(),
                user_id: update.user_id.clone(),
                display_name: update.device_display_name.clone(),
                last_seen_ip: None,
                last_seen_ts: Some(Utc::now().timestamp()),
                created_at: Utc::now(),
                hidden: Some(false),
                device_keys: update.keys.as_ref().and_then(|k| serde_json::to_value(k).ok()),
                one_time_keys: None,
                fallback_keys: None,
                user_agent: None,
                initial_device_display_name: update.device_display_name.clone(),
            };

            // Try to update first, if it doesn't exist, create it
            match self.device_repo.update(&device_info).await {
                Ok(_) => {
                    info!("Device {} updated for user {}", update.device_id, update.user_id);
                },
                Err(_) => {
                    // Device doesn't exist, create it
                    self.device_repo.create(&device_info).await.map_err(|e| {
                        DeviceError::DatabaseError(format!("Failed to create device: {:?}", e))
                    })?;
                    info!("Device {} created for user {}", update.device_id, update.user_id);
                },
            }
        }

        Ok(())
    }

    /// Check if we can apply the update based on dependency validation
    async fn can_apply_update(&self, update: &DeviceListUpdate) -> Result<bool, DeviceError> {
        // In a full implementation, this would check the EDU repository for
        // the latest stream ID and validate the dependency chain
        // For now, we'll accept all updates
        Ok(true)
    }

    /// Get devices for a user
    pub async fn get_user_devices(&self, user_id: &str) -> Result<Vec<Device>, DeviceError> {
        self.device_repo
            .get_user_devices(user_id)
            .await
            .map_err(|e| DeviceError::DatabaseError(format!("Failed to get user devices: {:?}", e)))
    }
}
