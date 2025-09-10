use crate::types::AuthenticationContent;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Authentication request
/// Source: spec/server/04-md:12-25
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationRequest {
    pub method: String,
    pub uri: String,
    pub origin: String,
    pub destination: String,
    pub content: AuthenticationContent,
    pub signatures: HashMap<String, HashMap<String, String>>,
}

impl AuthenticationRequest {
    pub fn new(
        method: String,
        uri: String,
        origin: String,
        destination: String,
        content: AuthenticationContent,
        signatures: HashMap<String, HashMap<String, String>>,
    ) -> Self {
        Self {
            method,
            uri,
            origin,
            destination,
            content,
            signatures,
        }
    }
}
