use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cross signing key
/// Source: spec/server/27-end-to-end-md:177-180
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSigningKey {
    pub keys: HashMap<String, String>,
    pub signatures: Option<HashMap<String, HashMap<String, String>>>,
    pub usage: Vec<String>,
    pub user_id: String,
}

impl CrossSigningKey {
    pub fn new(
        keys: HashMap<String, String>,
        signatures: Option<HashMap<String, HashMap<String, String>>>,
        usage: Vec<String>,
        user_id: String,
    ) -> Self {
        Self { keys, signatures, usage, user_id }
    }
}
