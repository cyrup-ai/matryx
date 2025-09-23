use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::federation::device_management::{DeviceError, DeviceListUpdate};
use matryx_entity::types::{Device, EDU, EphemeralEvent, EventContent};
use matryx_surrealdb::repository::{DeviceRepository, EDURepository, RepositoryError};

/// EDU (Ephemeral Data Unit) for device list updates
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceListUpdateEDU {
    pub edu_type: String, // "m.device_list_update"
    pub content: DeviceListUpdate,
}

/// EDU for signing key updates
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SigningKeyUpdateEDU {
    pub edu_type: String, // "m.signing_key_update"
    pub content: SigningKeyUpdateContent,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SigningKeyUpdateContent {
    pub user_id: String,
    pub master_key: Option<serde_json::Value>,
    pub self_signing_key: Option<serde_json::Value>,
}

/// Handler for processing device-related EDUs
pub struct DeviceEDUHandler {
    edu_repo: Arc<EDURepository<surrealdb::engine::any::Any>>,
    device_repo: Arc<DeviceRepository>,
}

impl DeviceEDUHandler {
    pub fn new(
        edu_repo: Arc<EDURepository<surrealdb::engine::any::Any>>,
        device_repo: Arc<DeviceRepository>,
    ) -> Self {
        Self { edu_repo, device_repo }
    }

    /// Process a device list update EDU
    pub async fn handle_device_list_update(
        &self,
        edu: DeviceListUpdateEDU,
    ) -> Result<(), DeviceError> {
        let user_id = &edu.content.user_id;

        // Store EDU in database
        let edu_content =
            EventContent::unknown(serde_json::to_value(&edu.content).map_err(|e| {
                DeviceError::DatabaseError(format!("JSON serialization error: {}", e))
            })?);

        let ephemeral_event = EphemeralEvent::new(
            edu_content,
            edu.edu_type.clone(),
            None, // Device list updates are not room-specific
            user_id.clone(),
        );

        let edu_entity = EDU::new(ephemeral_event, false);

        self.edu_repo
            .create(&edu_entity)
            .await
            .map_err(|e| DeviceError::DatabaseError(format!("Failed to store EDU: {:?}", e)))?;

        // Apply device update to repository
        if edu.content.deleted {
            self.device_repo.delete(&edu.content.device_id).await.map_err(|e| {
                DeviceError::DatabaseError(format!("Failed to delete device: {:?}", e))
            })?;
            info!("Device {} deleted for user {}", edu.content.device_id, user_id);
        } else {
            let device = Device {
                device_id: edu.content.device_id.clone(),
                user_id: edu.content.user_id.clone(),
                display_name: edu.content.device_display_name.clone(),
                last_seen_ip: None,
                last_seen_ts: Some(Utc::now().timestamp()),
                created_at: Utc::now(),
                hidden: Some(false),
                device_keys: edu.content.keys.as_ref().and_then(|k| serde_json::to_value(k).ok()),
                one_time_keys: None,
                fallback_keys: None,
                user_agent: None,
                initial_device_display_name: edu.content.device_display_name.clone(),
            };

            // Try to update first, if it doesn't exist, create it
            match self.device_repo.update(&device).await {
                Ok(_) => {
                    info!("Device {} updated for user {}", edu.content.device_id, user_id);
                },
                Err(_) => {
                    // Device doesn't exist, create it
                    self.device_repo.create(&device).await.map_err(|e| {
                        DeviceError::DatabaseError(format!("Failed to create device: {:?}", e))
                    })?;
                    info!("Device {} created for user {}", edu.content.device_id, user_id);
                },
            }
        }

        Ok(())
    }

    /// Process a signing key update EDU
    pub async fn handle_signing_key_update(
        &self,
        edu: SigningKeyUpdateEDU,
    ) -> Result<(), DeviceError> {
        let user_id = &edu.content.user_id;

        // Store EDU in database
        let edu_content =
            EventContent::unknown(serde_json::to_value(&edu.content).map_err(|e| {
                DeviceError::DatabaseError(format!("JSON serialization error: {}", e))
            })?);

        let ephemeral_event = EphemeralEvent::new(
            edu_content,
            edu.edu_type.clone(),
            None, // Signing key updates are not room-specific
            user_id.clone(),
        );

        let edu_entity = EDU::new(ephemeral_event, false);

        self.edu_repo
            .create(&edu_entity)
            .await
            .map_err(|e| DeviceError::DatabaseError(format!("Failed to store EDU: {:?}", e)))?;

        info!("Updated signing keys for user {}", user_id);
        Ok(())
    }

    /// Get device list for a user from repository
    pub async fn get_user_devices(&self, user_id: &str) -> Result<Vec<Device>, DeviceError> {
        self.device_repo
            .get_user_devices(user_id)
            .await
            .map_err(|e| DeviceError::DatabaseError(format!("Failed to get user devices: {:?}", e)))
    }

    /// Clear devices for a user (useful for forcing resync)
    pub async fn clear_user_devices(&self, user_id: &str) -> Result<(), DeviceError> {
        self.device_repo.delete_user_devices(user_id).await.map_err(|e| {
            DeviceError::DatabaseError(format!("Failed to clear user devices: {:?}", e))
        })?;
        info!("Cleared devices for user {}", user_id);
        Ok(())
    }

    /// Get device statistics from repository
    pub async fn get_device_stats(&self) -> Result<DeviceStats, DeviceError> {
        // Note: This is a simplified implementation that doesn't provide exact statistics
        // In a production system, you would implement a more efficient query in the repository
        info!("Device statistics requested - returning placeholder stats");

        Ok(DeviceStats {
            total_users: 0,
            total_devices: 0,
            users_with_devices: Vec::new(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DeviceStats {
    pub total_users: usize,
    pub total_devices: usize,
    pub users_with_devices: Vec<String>,
}

/// Process EDU based on type
pub async fn process_edu(
    edu_type: &str,
    content: serde_json::Value,
    handler: &DeviceEDUHandler,
) -> Result<(), DeviceError> {
    match edu_type {
        "m.device_list_update" => {
            let device_update_edu = DeviceListUpdateEDU {
                edu_type: edu_type.to_string(),
                content: serde_json::from_value(content)?,
            };
            handler.handle_device_list_update(device_update_edu).await
        },
        "m.signing_key_update" => {
            let signing_key_edu = SigningKeyUpdateEDU {
                edu_type: edu_type.to_string(),
                content: serde_json::from_value(content)?,
            };
            handler.handle_signing_key_update(signing_key_edu).await
        },
        _ => {
            // Unknown EDU type, ignore
            info!("Ignoring unknown EDU type: {}", edu_type);
            Ok(())
        },
    }
}
