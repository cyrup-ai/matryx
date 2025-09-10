use serde::{Deserialize, Serialize};

/// Rate limit response
/// Source: spec/client/02_rooms_md:418-420
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitResponse {
    pub errcode: String,
    pub error: String,
    pub retry_after_ms: i64,
}

impl RateLimitResponse {
    pub fn new(errcode: String, error: String, retry_after_ms: i64) -> Self {
        Self { errcode, error, retry_after_ms }
    }
}
