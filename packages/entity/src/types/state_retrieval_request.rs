use serde::{Deserialize, Serialize};

/// State retrieval request
/// Source: spec/server/23-retrieving-md:101
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateRetrievalRequest {
    pub event_id: String,
}

impl StateRetrievalRequest {
    pub fn new(event_id: String) -> Self {
        Self { event_id }
    }
}
