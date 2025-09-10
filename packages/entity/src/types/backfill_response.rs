use crate::types::PDU;
use serde::{Deserialize, Serialize};

/// BackfillResponse
/// Source: spec/server/22-backfill-md:52-63
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillResponse {
    pub origin: String,
    pub origin_server_ts: i64,
    pub pdus: Vec<PDU>,
}

impl BackfillResponse {
    pub fn new(origin: String, origin_server_ts: i64, pdus: Vec<PDU>) -> Self {
        Self { origin, origin_server_ts, pdus }
    }
}
