use serde::{Deserialize, Serialize};

/// Chunk of published rooms information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishedRoomsChunk {
    /// Room ID
    pub room_id: String,

    /// Room name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Room topic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,

    /// Room canonical alias
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_alias: Option<String>,

    /// Number of joined members
    pub num_joined_members: i64,

    /// Room avatar URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,

    /// Whether the room is world readable
    pub world_readable: bool,

    /// Whether guests can join
    pub guest_can_join: bool,

    /// Room type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_type: Option<String>,
}

impl PublishedRoomsChunk {
    pub fn new(
        room_id: String,
        num_joined_members: i64,
        world_readable: bool,
        guest_can_join: bool,
    ) -> Self {
        Self {
            room_id,
            name: None,
            topic: None,
            canonical_alias: None,
            num_joined_members,
            avatar_url: None,
            world_readable,
            guest_can_join,
            room_type: None,
        }
    }
}
