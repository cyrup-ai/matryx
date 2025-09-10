use crate::types::StrippedStateEvent;
use serde::{Deserialize, Serialize};

/// SpaceHierarchyChildRoomsChunk
/// Source: spec/server/13-public-md:318-345
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceHierarchyChildRoomsChunk {
    pub allowed_room_ids: Option<Vec<String>>,
    pub avatar_url: Option<String>,
    pub canonical_alias: Option<String>,
    pub children_state: Vec<StrippedStateEvent>,
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
    pub fn new(
        allowed_room_ids: Option<Vec<String>>,
        avatar_url: Option<String>,
        canonical_alias: Option<String>,
        children_state: Vec<StrippedStateEvent>,
        encryption: Option<String>,
        guest_can_join: bool,
        join_rule: Option<String>,
        name: Option<String>,
        num_joined_members: i64,
        room_id: String,
        room_type: Option<String>,
        room_version: Option<String>,
        topic: Option<String>,
        world_readable: bool,
    ) -> Self {
        Self {
            allowed_room_ids,
            avatar_url,
            canonical_alias,
            children_state,
            encryption,
            guest_can_join,
            join_rule,
            name,
            num_joined_members,
            room_id,
            room_type,
            room_version,
            topic,
            world_readable,
        }
    }
}
