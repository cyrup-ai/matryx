use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Key object
/// Source: spec/server/27-end-to-end-md:75-76
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyObject {
    pub key: String,
    pub signatures: HashMap<String, HashMap<String, String>>,
}

impl KeyObject {
    pub fn new(key: String, signatures: HashMap<String, HashMap<String, String>>) -> Self {
        Self { key, signatures }
    }
}
