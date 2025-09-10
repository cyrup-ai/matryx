use crate::types::PublicRoomsFilter;
use serde::{Deserialize, Serialize};

/// PublicRoomsPostRequest
/// Source: spec/server/13-public-md:120-135
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomsPostRequest {
    pub filter: Option<PublicRoomsFilter>,
    pub include_all_networks: Option<bool>,
    pub limit: Option<i64>,
    pub since: Option<String>,
    pub third_party_instance_id: Option<String>,
}

impl PublicRoomsPostRequest {
    pub fn new(
        filter: Option<PublicRoomsFilter>,
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
