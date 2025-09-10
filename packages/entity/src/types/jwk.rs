use serde::{Deserialize, Serialize};

/// JWK (JSON Web Key)
/// Source: spec/client/04_security_md:619-624
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JWK {
    pub kty: String,
    pub key_ops: Vec<String>,
    pub alg: String,
    pub k: String,
    pub ext: bool,
}

impl JWK {
    pub fn new(kty: String, key_ops: Vec<String>, alg: String, k: String, ext: bool) -> Self {
        Self { kty, key_ops, alg, k, ext }
    }
}
