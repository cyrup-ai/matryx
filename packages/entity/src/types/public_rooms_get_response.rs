use crate::types::PublishedRoomsChunk;
use serde::{Deserialize, Serialize};

/// PublicRoomsGetResponse
/// Source: spec/server/13-public-md:45-55
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicRoomsGetResponse {
    pub chunk: Vec<PublishedRoomsChunk>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
    pub total_room_count_estimate: Option<i64>,
}

impl PublicRoomsGetResponse {
    pub fn new(
        chunk: Vec<PublishedRoomsChunk>,
        next_batch: Option<String>,
        prev_batch: Option<String>,
        total_room_count_estimate: Option<i64>,
    ) -> Self {
        Self {
            chunk,
            next_batch,
            prev_batch,
            total_room_count_estimate,
        }
    }
}
