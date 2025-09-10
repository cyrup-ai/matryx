use crate::types::DeviceKeys;
use serde::{Deserialize, Serialize};

/// DeviceListUpdate
/// Source: spec/server/07-md:134-144
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceListUpdate {
    pub device_display_name: Option<String>,
    pub device_id: String,
    pub deleted: Option<bool>,
    pub keys: Option<DeviceKeys>,
    pub prev_id: Vec<String>,
    pub stream_id: i64,
    pub user_id: String,
}

impl DeviceListUpdate {
    pub fn new(
        device_display_name: Option<String>,
        device_id: String,
        deleted: Option<bool>,
        keys: Option<DeviceKeys>,
        prev_id: Vec<String>,
        stream_id: i64,
        user_id: String,
    ) -> Self {
        Self {
            device_display_name,
            device_id,
            deleted,
            keys,
            prev_id,
            stream_id,
            user_id,
        }
    }
}
