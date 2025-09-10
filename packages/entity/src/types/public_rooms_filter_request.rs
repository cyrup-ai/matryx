use crate::types::Filter;
use serde::{Deserialize, Serialize};

/// PublicRoomsFilterRequest
/// Source: spec/server/13-public-md:175-185
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomsFilterRequest {
    pub filter: Option<Filter>,
    pub include_all_networks: Option<bool>,
    pub limit: Option<i64>,
    pub since: Option<String>,
    pub third_party_instance_id: Option<String>,
}

impl PublicRoomsFilterRequest {
    pub fn new(
        filter: Option<Filter>,
        include_all_networks: Option<bool>,
        limit: Option<i64>,
        since: Option<String>,
        third_party_instance_id: Option<String>,
    ) -> Self {
        Self {
            filter,
            include_all_networks,
            limit,
            since,
            third_party_instance_id,
        }
    }
}
