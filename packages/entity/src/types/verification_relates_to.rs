use serde::{Deserialize, Serialize};

/// Verification relates to for Matrix verification events
/// Represents the m.relates_to field in verification events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationRelatesTo {
    pub rel_type: String,
    pub event_id: String,
}

impl VerificationRelatesTo {
    pub fn new(rel_type: String, event_id: String) -> Self {
        Self { rel_type, event_id }
    }

    pub fn reference(event_id: String) -> Self {
        Self { rel_type: "m.reference".to_string(), event_id }
    }
}
