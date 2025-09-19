use crate::types::SpaceChildEvent;
use serde::{Deserialize, Serialize};

/// Parameters for creating a space hierarchy room
#[derive(Debug, Clone)]
pub struct SpaceHierarchyRoomParams {
    pub avatar_url: Option<String>,
    pub canonical_alias: Option<String>,
    pub children_state: Vec<SpaceChildEvent>,
    pub guest_can_join: bool,
    pub join_rule: String,
    pub name: Option<String>,
    pub num_joined_members: i64,
    pub room_id: String,
    pub room_type: Option<String>,
    pub topic: Option<String>,
    pub world_readable: bool,
}

/// SpaceHierarchyRoom
/// Source: spec/client/07_relationship_md:290-338
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceHierarchyRoom {
    pub avatar_url: Option<String>,
    pub canonical_alias: Option<String>,
    pub children_state: Vec<SpaceChildEvent>,
    pub guest_can_join: bool,
    pub join_rule: String,
    pub name: Option<String>,
    pub num_joined_members: i64,
    pub room_id: String,
    pub room_type: Option<String>,
    pub topic: Option<String>,
    pub world_readable: bool,
}

impl SpaceHierarchyRoom {
    pub fn new(params: SpaceHierarchyRoomParams) -> Self {
        Self {
            avatar_url: params.avatar_url,
            canonical_alias: params.canonical_alias,
            children_state: params.children_state,
            guest_can_join: params.guest_can_join,
            join_rule: params.join_rule,
            name: params.name,
            num_joined_members: params.num_joined_members,
            room_id: params.room_id,
            room_type: params.room_type,
            topic: params.topic,
            world_readable: params.world_readable,
        }
    }
}
