use crate::types::{EDU, PDU};
use serde::{Deserialize, Serialize};

/// FederationTransaction
/// Source: spec/server/05-md:22-35
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationTransaction {
    pub origin: String,
    pub origin_server_ts: i64,
    pub pdus: Vec<PDU>,
    pub edus: Option<Vec<EDU>>,
}

impl FederationTransaction {
    pub fn new(
        origin: String,
        origin_server_ts: i64,
        pdus: Vec<PDU>,
        edus: Option<Vec<EDU>>,
    ) -> Self {
        Self { origin, origin_server_ts, pdus, edus }
    }
}
