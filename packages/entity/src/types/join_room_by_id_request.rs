use crate::types::ThirdPartySigned;
use serde::{Deserialize, Serialize};

/// Join room by ID request
/// Source: spec/client/02_rooms_md:751-752
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinRoomByIdRequest {
    pub reason: Option<String>,
    pub third_party_signed: Option<ThirdPartySigned>,
}

impl JoinRoomByIdRequest {
    pub fn new(reason: Option<String>, third_party_signed: Option<ThirdPartySigned>) -> Self {
        Self { reason, third_party_signed }
    }
}
