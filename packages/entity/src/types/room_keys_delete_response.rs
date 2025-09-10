use serde::{Deserialize, Serialize};

/// RoomKeysDeleteResponse
/// Source: spec/client/04_security_md:2245-2247
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeysDeleteResponse {
    pub count: i64,
    pub etag: String,
}

impl RoomKeysDeleteResponse {
    pub fn new(count: i64, etag: String) -> Self {
        Self { count, etag }
    }
}
