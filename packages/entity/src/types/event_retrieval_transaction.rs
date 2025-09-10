use crate::types::PDU;
use serde::{Deserialize, Serialize};

/// Event retrieval transaction
/// Source: spec/server/23-retrieving-md:47-49
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRetrievalTransaction {
    pub origin: String,
    pub origin_server_ts: i64,
    pub pdus: Vec<PDU>,
}

impl EventRetrievalTransaction {
    pub fn new(origin: String, origin_server_ts: i64, pdus: Vec<PDU>) -> Self {
        Self { origin, origin_server_ts, pdus }
    }
}
