use crate::types::Event;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub latest_event: Option<Event>,
    pub count: usize,
    pub participated: bool,
    pub participants: Vec<String>,
}
