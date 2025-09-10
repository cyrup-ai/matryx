use crate::types::{CrossSigningKey, DeviceInfo};
use serde::{Deserialize, Serialize};

/// DeviceListResponse
/// Source: spec/server/17-device-md:34-58
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceListResponse {
    pub devices: Vec<DeviceInfo>,
    pub master_key: Option<CrossSigningKey>,
    pub self_signing_key: Option<CrossSigningKey>,
    pub stream_id: i64,
    pub user_id: String,
}

impl DeviceListResponse {
    pub fn new(
        devices: Vec<DeviceInfo>,
        master_key: Option<CrossSigningKey>,
        self_signing_key: Option<CrossSigningKey>,
        stream_id: i64,
        user_id: String,
    ) -> Self {
        Self {
            devices,
            master_key,
            self_signing_key,
            stream_id,
            user_id,
        }
    }
}
