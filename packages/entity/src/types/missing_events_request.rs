use serde::{Deserialize, Serialize};

/// MissingEventsRequest
/// Source: spec/server/22-backfill-md:97-107
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingEventsRequest {
    pub earliest_events: Vec<String>,
    pub latest_events: Vec<String>,
    pub limit: Option<i64>,
    pub min_depth: Option<i64>,
}

impl MissingEventsRequest {
    pub fn new(
        earliest_events: Vec<String>,
        latest_events: Vec<String>,
        limit: Option<i64>,
        min_depth: Option<i64>,
    ) -> Self {
        Self { earliest_events, latest_events, limit, min_depth }
    }
}
