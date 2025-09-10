use crate::types::InviteEventContainer;
use serde::{Deserialize, Serialize};

/// InviteV1Response
/// Source: spec/server/11-room-md:130-135
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteV1Response {
    pub response: Vec<(i64, InviteEventContainer)>,
}

impl InviteV1Response {
    pub fn new(response: Vec<(i64, InviteEventContainer)>) -> Self {
        Self { response }
    }
}
