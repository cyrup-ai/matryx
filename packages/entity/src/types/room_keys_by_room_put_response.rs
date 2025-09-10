use serde::{Deserialize, Serialize};

/// RoomKeysByRoomPutResponse
/// Source: spec/client/04_security_md:2455-2457
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeysByRoomPutResponse {
    pub count: i64,
    pub etag: String,
}

impl RoomKeysByRoomPutResponse {
    pub fn new(count: i64, etag: String) -> Self {
        Self { count, etag }
    }
}
