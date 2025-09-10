use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Third party signed
/// Source: spec/client/02_rooms_md:624-627
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThirdPartySigned {
    pub mxid: String,
    pub sender: String,
    pub signatures: HashMap<String, HashMap<String, String>>,
    pub token: String,
}

impl ThirdPartySigned {
    pub fn new(
        mxid: String,
        sender: String,
        signatures: HashMap<String, HashMap<String, String>>,
        token: String,
    ) -> Self {
        Self { mxid, sender, signatures, token }
    }
}
