use crate::types::{SpaceHierarchyChildRoomsChunk, SpaceHierarchyParentRoom};
use serde::{Deserialize, Serialize};

/// SpaceHierarchyResponse
/// Source: spec/server/13-public-md:300-316
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceHierarchyResponse {
    pub children: Vec<SpaceHierarchyChildRoomsChunk>,
    pub inaccessible_children: Vec<String>,
    pub room: SpaceHierarchyParentRoom,
}

impl SpaceHierarchyResponse {
    pub fn new(
        children: Vec<SpaceHierarchyChildRoomsChunk>,
        inaccessible_children: Vec<String>,
        room: SpaceHierarchyParentRoom,
    ) -> Self {
        Self { children, inaccessible_children, room }
    }
}
