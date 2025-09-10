//! Matrix Event Utilities
//!
//! Shared utilities for Matrix event processing including content hashing,
//! event signing, and canonical JSON serialization according to Matrix specification.

use crate::state::AppState;
use base64::{Engine, engine::general_purpose};
use matryx_entity::types::Event;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Calculate SHA256 content hashes for Matrix event according to specification
///
/// Creates canonical JSON representation of event content and calculates
/// SHA256 hash following Matrix specification requirements for event integrity.
pub fn calculate_content_hashes(
    event: &Event,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    // Create canonical JSON for content hashing per Matrix specification
    let canonical_content = json!({
        "auth_events": event.auth_events,
        "content": event.content,
        "depth": event.depth,
        "event_type": event.event_type,
        "prev_events": event.prev_events,
        "room_id": event.room_id,
        "sender": event.sender,
        "state_key": event.state_key,
        "origin_server_ts": event.origin_server_ts
    });

    // Convert to canonical JSON string (sorted keys, no whitespace)
    let canonical_json = to_canonical_json(&canonical_content)?;

    // Calculate SHA256 hash
    let mut hasher = Sha256::new();
    hasher.update(canonical_json.as_bytes());
    let hash = hasher.finalize();

    // Encode as base64
    let hash_b64 = general_purpose::STANDARD.encode(&hash);

    Ok(json!({
        "sha256": hash_b64
    }))
}

/// Sign Matrix event with server's Ed25519 private key according to specification
///
/// Creates canonical JSON representation and signs with server's private key
/// following Matrix federation signature requirements for event authentication.
pub async fn sign_event(
    state: &AppState,
    event: &Event,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    use ed25519_dalek::{Signer, SigningKey};

    // Get server signing key from database
    let query = "
        SELECT private_key, key_id 
        FROM server_signing_keys 
        WHERE server_name = $server_name 
          AND is_active = true 
        ORDER BY created_at DESC 
        LIMIT 1
    ";

    let mut response = state
        .db
        .query(query)
        .bind(("server_name", state.homeserver_name.clone()))
        .await?;

    #[derive(serde::Deserialize)]
    struct SigningKeyRecord {
        private_key: String,
        key_id: String,
    }

    let key_record: Option<SigningKeyRecord> = response.take(0)?;
    let key_record = key_record.ok_or("No active signing key found for server")?;

    // Create canonical JSON for signing per Matrix specification
    let canonical_event = json!({
        "auth_events": event.auth_events,
        "content": event.content,
        "depth": event.depth,
        "event_type": event.event_type,
        "hashes": event.hashes,
        "prev_events": event.prev_events,
        "room_id": event.room_id,
        "sender": event.sender,
        "state_key": event.state_key,
        "origin_server_ts": event.origin_server_ts
    });

    // Convert to canonical JSON string
    let canonical_json = to_canonical_json(&canonical_event)?;

    // Decode private key from base64
    let private_key_bytes = general_purpose::STANDARD.decode(&key_record.private_key)?;

    // Validate key length
    if private_key_bytes.len() != 32 {
        return Err("Invalid private key length for Ed25519".into());
    }

    // Create Ed25519 signing key
    let private_key_array: [u8; 32] = private_key_bytes
        .try_into()
        .map_err(|_| "Failed to convert private key to array")?;
    let signing_key = SigningKey::from_bytes(&private_key_array);

    // Sign canonical JSON
    let signature = signing_key.sign(canonical_json.as_bytes());
    let signature_b64 = general_purpose::STANDARD.encode(signature.to_bytes());

    Ok(json!({
        state.homeserver_name.clone(): {
            key_record.key_id: signature_b64
        }
    }))
}

/// Convert JSON value to Matrix canonical JSON string with sorted keys
///
/// Implements Matrix canonical JSON as defined in the Matrix specification:
/// - Object keys sorted in lexicographic order
/// - No insignificant whitespace
/// - UTF-8 encoding
/// - Numbers in shortest form
///
/// This is critical for signature verification and hash calculation to work
/// correctly with other Matrix homeservers.
pub fn to_canonical_json(
    value: &Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    match value {
        Value::Null => Ok("null".to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) => {
            // JSON string with proper escaping
            Ok(serde_json::to_string(s)?)
        },
        Value::Array(arr) => {
            let elements: Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> =
                arr.iter().map(|v| to_canonical_json(v)).collect();
            Ok(format!("[{}]", elements?.join(",")))
        },
        Value::Object(obj) => {
            // Sort keys lexicographically (critical for Matrix signature verification)
            let mut sorted_keys: Vec<&String> = obj.keys().collect();
            sorted_keys.sort();

            let pairs: Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> = sorted_keys
                .into_iter()
                .map(|key| {
                    let key_json = serde_json::to_string(key)?;
                    let value_json = to_canonical_json(&obj[key])?;
                    Ok(format!("{}:{}", key_json, value_json))
                })
                .collect();

            Ok(format!("{{{}}}", pairs?.join(",")))
        },
    }
}
