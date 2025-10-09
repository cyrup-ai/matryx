use crate::types::Event;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub latest_event: Option<Event>,
    pub count: usize,
    pub participated: bool,
    pub participants: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight_count: Option<usize>,
}
