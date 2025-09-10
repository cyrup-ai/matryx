use serde::{Deserialize, Serialize};

/// HistoryVisibilityEvent
/// Source: spec/client/05_advanced_md:47
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryVisibilityEvent {
    pub history_visibility: String,
}

impl HistoryVisibilityEvent {
    pub fn new(history_visibility: String) -> Self {
        Self { history_visibility }
    }
}
