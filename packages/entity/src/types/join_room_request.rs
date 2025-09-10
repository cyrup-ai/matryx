use crate::types::ThirdPartySigned;
use serde::{Deserialize, Serialize};

/// Join room request
/// Source: spec/client/02_rooms_md:619-620
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRoomRequest {
    pub reason: Option<String>,
    pub third_party_signed: Option<ThirdPartySigned>,
}

impl JoinRoomRequest {
    pub fn new(reason: Option<String>, third_party_signed: Option<ThirdPartySigned>) -> Self {
        Self { reason, third_party_signed }
    }
}
