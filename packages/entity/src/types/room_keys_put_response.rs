use serde::{Deserialize, Serialize};

/// RoomKeysPutResponse
/// Source: spec/client/04_security_md:2070-2072
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomKeysPutResponse {
    pub count: i64,
    pub etag: String,
}

impl RoomKeysPutResponse {
    pub fn new(count: i64, etag: String) -> Self {
        Self { count, etag }
    }
}
