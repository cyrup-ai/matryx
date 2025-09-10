use crate::types::PDU;
use serde::{Deserialize, Serialize};

/// MissingEventsResponse
/// Source: spec/server/22-backfill-md:138-148
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingEventsResponse {
    pub events: Vec<PDU>,
}

impl MissingEventsResponse {
    pub fn new(events: Vec<PDU>) -> Self {
        Self { events }
    }
}
