use serde::{Deserialize, Serialize};

/// SendLeaveV2Response - Empty response indicating event acceptance
/// Source: spec/server/10-room-leaves.md:218-224
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendLeaveV2Response {
    // Empty struct for empty JSON response {}
}

impl SendLeaveV2Response {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SendLeaveV2Response {
    fn default() -> Self {
        Self::new()
    }
}
