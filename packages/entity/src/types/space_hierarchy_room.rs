use crate::types::SpaceChildEvent;
use serde::{Deserialize, Serialize};

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
    pub fn new(
        avatar_url: Option<String>,
        canonical_alias: Option<String>,
        children_state: Vec<SpaceChildEvent>,
        guest_can_join: bool,
        join_rule: String,
        name: Option<String>,
        num_joined_members: i64,
        room_id: String,
        room_type: Option<String>,
        topic: Option<String>,
        world_readable: bool,
    ) -> Self {
        Self {
            avatar_url,
            canonical_alias,
            children_state,
            guest_can_join,
            join_rule,
            name,
            num_joined_members,
            room_id,
            room_type,
            topic,
            world_readable,
        }
    }
}
