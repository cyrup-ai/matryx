use serde::{Deserialize, Serialize};

/// Query criteria
/// Source: spec/server/03-server-md:97-98
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryCriteria {
    pub minimum_valid_until_ts: Option<i64>,
}

impl QueryCriteria {
    pub fn new(minimum_valid_until_ts: Option<i64>) -> Self {
        Self { minimum_valid_until_ts }
    }
}
