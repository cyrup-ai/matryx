use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// BackedUpSessionData
/// Source: spec/client/04_security_md:1910-1915
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackedUpSessionData {
    pub algorithm: String,
    pub forwarding_curve25519_key_chain: Vec<String>,
    pub sender_claimed_keys: HashMap<String, String>,
    pub sender_key: String,
    pub session_key: String,
}

impl BackedUpSessionData {
    pub fn new(
        algorithm: String,
        forwarding_curve25519_key_chain: Vec<String>,
        sender_claimed_keys: HashMap<String, String>,
        sender_key: String,
        session_key: String,
    ) -> Self {
        Self {
            algorithm,
            forwarding_curve25519_key_chain,
            sender_claimed_keys,
            sender_key,
            session_key,
        }
    }
}
