use crate::types::{EDU, PDU};
use serde::{Deserialize, Serialize};

/// Transaction
/// Source: spec/server/20-transaction-md:14-16
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub origin: String,
    pub origin_server_ts: i64,
    pub pdus: Vec<PDU>,
    pub edus: Vec<EDU>,
}

impl Transaction {
    pub fn new(origin: String, origin_server_ts: i64, pdus: Vec<PDU>, edus: Vec<EDU>) -> Self {
        Self { origin, origin_server_ts, pdus, edus }
    }
}
