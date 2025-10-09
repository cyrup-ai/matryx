use crate::types::SpaceHierarchyStrippedStateEvent;
use serde::{Deserialize, Serialize};

/// Parameters for creating a space hierarchy child rooms chunk
#[derive(Debug, Clone)]
pub struct SpaceHierarchyChildRoomsParams {
    pub allowed_room_ids: Option<Vec<String>>,
    pub avatar_url: Option<String>,
    pub canonical_alias: Option<String>,
    pub children_state: Vec<SpaceHierarchyStrippedStateEvent>,
    pub encryption: Option<String>,
    pub guest_can_join: bool,
    pub join_rule: Option<String>,
    pub name: Option<String>,
    pub num_joined_members: i64,
    pub room_id: String,
    pub room_type: Option<String>,
    pub room_version: Option<String>,
    pub topic: Option<String>,
    pub world_readable: bool,
}

/// SpaceHierarchyChildRoomsChunk
/// Source: spec/server/13-public-md:318-345
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceHierarchyChildRoomsChunk {
    pub allowed_room_ids: Option<Vec<String>>,
    pub avatar_url: Option<String>,
    pub canonical_alias: Option<String>,
    pub children_state: Vec<SpaceHierarchyStrippedStateEvent>,
    pub encryption: Option<String>,
    pub guest_can_join: bool,
    pub join_rule: Option<String>,
    pub name: Option<String>,
    pub num_joined_members: i64,
    pub room_id: String,
    pub room_type: Option<String>,
    pub room_version: Option<String>,
    pub topic: Option<String>,
    pub world_readable: bool,
}

impl SpaceHierarchyChildRoomsChunk {
    pub fn new(params: SpaceHierarchyChildRoomsParams) -> Self {
        Self {
            allowed_room_ids: params.allowed_room_ids,
            avatar_url: params.avatar_url,
            canonical_alias: params.canonical_alias,
            children_state: params.children_state,
            encryption: params.encryption,
            guest_can_join: params.guest_can_join,
            join_rule: params.join_rule,
            name: params.name,
            num_joined_members: params.num_joined_members,
            room_id: params.room_id,
            room_type: params.room_type,
            room_version: params.room_version,
            topic: params.topic,
            world_readable: params.world_readable,
        }
    }
}
