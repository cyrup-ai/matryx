use crate::types::{OldVerifyKey, VerifyKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Server keys response
/// Source: spec/server/03-server-md:31-51
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerKeysResponse {
    pub old_verify_keys: Option<HashMap<String, OldVerifyKey>>,
    pub server_name: String,
    pub signatures: HashMap<String, HashMap<String, String>>,
    pub valid_until_ts: i64,
    pub verify_keys: HashMap<String, VerifyKey>,
}

impl ServerKeysResponse {
    pub fn new(
        old_verify_keys: Option<HashMap<String, OldVerifyKey>>,
        server_name: String,
        signatures: HashMap<String, HashMap<String, String>>,
        valid_until_ts: i64,
        verify_keys: HashMap<String, VerifyKey>,
    ) -> Self {
        Self {
            old_verify_keys,
            server_name,
            signatures,
            valid_until_ts,
            verify_keys,
        }
    }
}
