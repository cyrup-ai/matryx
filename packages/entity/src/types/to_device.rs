use crate::types::Event;
use serde::{Deserialize, Serialize};

/// To device
/// Source: spec/client/04_security_md:86
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToDevice {
    pub events: Vec<Event>,
}

impl ToDevice {
    pub fn new(events: Vec<Event>) -> Self {
        Self { events }
    }
}
