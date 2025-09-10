use crate::types::PublishedRoomsChunk;
use serde::{Deserialize, Serialize};

/// PublishedRoomsResponse
/// Source: spec/server/13-public-md:45-64
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedRoomsResponse {
    pub chunk: Vec<PublishedRoomsChunk>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
    pub total_room_count_estimate: Option<i64>,
}

impl PublishedRoomsResponse {
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
