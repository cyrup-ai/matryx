use serde::{Deserialize, Serialize};

fn default_event_format() -> String {
    "client".to_string()
}

/// Matrix-compliant filter for sync and events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_fields: Option<Vec<String>>,

    #[serde(default = "default_event_format")]
    pub event_format: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence: Option<EventFilter>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_data: Option<EventFilter>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub room: Option<RoomFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rooms: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_rooms: Option<Vec<String>>,

    #[serde(default)]
    pub include_leave: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeline: Option<RoomEventFilter>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<RoomEventFilter>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ephemeral: Option<RoomEventFilter>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_data: Option<RoomEventFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_types: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub senders: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_senders: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEventFilter {
    #[serde(flatten)]
    pub base: EventFilter,

    #[serde(default)]
    pub lazy_load_members: bool,

    #[serde(default)]
    pub include_redundant_members: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains_url: Option<bool>,
}

impl MatrixFilter {
    pub fn new() -> Self {
        Self {
            event_fields: None,
            event_format: default_event_format(),
            presence: None,
            account_data: None,
            room: None,
        }
    }
}

impl Default for MatrixFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventFilter {
    pub fn new() -> Self {
        Self {
            limit: None,
            types: None,
            not_types: None,
            senders: None,
            not_senders: None,
        }
    }
}

impl Default for EventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl RoomEventFilter {
    pub fn new() -> Self {
        Self {
            base: EventFilter::new(),
            lazy_load_members: false,
            include_redundant_members: false,
            contains_url: None,
        }
    }
}

impl Default for RoomEventFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl RoomFilter {
    pub fn new() -> Self {
        Self {
            rooms: None,
            not_rooms: None,
            include_leave: false,
            timeline: None,
            state: None,
            ephemeral: None,
            account_data: None,
        }
    }
}

impl Default for RoomFilter {
    fn default() -> Self {
        Self::new()
    }
}
