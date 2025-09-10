use crate::types::{QueryRequest, QueryResponse};
use serde::{Deserialize, Serialize};

/// Query
/// Source: spec/server/01-md:19-20
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub request: QueryRequest,
    pub response: QueryResponse,
    pub snapshot_state: bool,
}

impl Query {
    pub fn new(request: QueryRequest, response: QueryResponse, snapshot_state: bool) -> Self {
        Self { request, response, snapshot_state }
    }
}
