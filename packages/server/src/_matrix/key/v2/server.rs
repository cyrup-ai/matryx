use axum::{Json, extract::State, http::StatusCode};
use base64::{Engine, engine::general_purpose};
use serde_json::{Value, json};
use std::env;
use tracing::{error, info};

use crate::AppState;
use crate::utils::canonical_json::to_canonical_json;

#[derive(serde::Deserialize)]
struct SigningKeyRecord {
    key_id: String,
    public_key: String,
    private_key: String,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// GET /_matrix/key/v2/server
///
/// Returns the homeserver's published signing keys for federation.
/// Other servers use these keys to verify signatures on events and requests.
pub async fn get(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // Get server name from environment with proper error handling
    let server_name = env::var("HOMESERVER_NAME").map_err(|_| {
        error!("HOMESERVER_NAME environment variable not set");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Get or generate Ed25519 signing keys from database
    let (verify_keys, old_verify_keys, signatures) =
        get_or_generate_signing_keys(&state, &server_name).await.map_err(|e| {
            error!("Failed to get signing keys: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let current_time_ms = chrono::Utc::now().timestamp_millis();
    let valid_until_ms = current_time_ms + (7 * 24 * 60 * 60 * 1000); // 7 days from now

    Ok(Json(json!({
        "server_name": server_name,
        "verify_keys": verify_keys,
        "old_verify_keys": old_verify_keys,
        "signatures": signatures,
        "valid_until_ts": valid_until_ms
    })))
}

/// Get existing signing keys from database or generate new ones
async fn get_or_generate_signing_keys(
    state: &AppState,
    server_name: &str,
) -> Result<(Value, Value, Value), Box<dyn std::error::Error + Send + Sync>> {
    // Query existing signing keys
    let query = "
        SELECT key_id, public_key, private_key, created_at, expires_at
        FROM server_signing_keys
        WHERE server_name = $server_name
          AND is_active = true
          AND (expires_at IS NULL OR expires_at > datetime::now())
        ORDER BY created_at DESC
    ";

    let mut response = state
        .db
        .query(query)
        .bind(("server_name", server_name.to_string()))
        .await?;

    let existing_keys: Vec<SigningKeyRecord> = response.take(0)?;

    let (current_key, old_keys): (SigningKeyRecord, Vec<SigningKeyRecord>) = if existing_keys
        .is_empty()
    {
        // No keys exist, generate new ones
        info!("No signing keys found for server {}, generating new Ed25519 key pair", server_name);
        let new_key = generate_ed25519_keypair(state, server_name).await?;
        (new_key, vec![])
    } else {
        // Use existing keys
        let current = existing_keys
            .into_iter()
            .next()
            .ok_or("No current key found despite non-empty keys")?;
        (current, vec![])
    };

    // Build verify_keys JSON
    let mut verify_keys = serde_json::Map::new();
    verify_keys.insert(
        current_key.key_id.clone(),
        json!({
            "key": current_key.public_key
        }),
    );

    // Build old_verify_keys JSON (empty for now)
    let old_verify_keys = json!({});

    // Build signatures JSON
    let canonical_json = build_canonical_server_json(server_name, &verify_keys)?;
    let signature = sign_canonical_json(&canonical_json, &current_key.private_key)?;

    let mut server_signatures = serde_json::Map::new();
    server_signatures.insert(current_key.key_id.clone(), json!(signature));

    let mut signatures = serde_json::Map::new();
    signatures.insert(server_name.to_string(), json!(server_signatures));

    Ok((json!(verify_keys), old_verify_keys, json!(signatures)))
}

/// Generate new Ed25519 keypair and store in database
async fn generate_ed25519_keypair(
    state: &AppState,
    server_name: &str,
) -> Result<SigningKeyRecord, Box<dyn std::error::Error + Send + Sync>> {
    use ed25519_dalek::SigningKey;
    use rand::{RngCore, rngs::OsRng};

    // Generate proper Ed25519 keypair using cryptographically secure random number generator
    let mut rng = OsRng;
    let mut secret_bytes = [0u8; 32];
    rng.fill_bytes(&mut secret_bytes);
    let signing_key = SigningKey::from_bytes(&secret_bytes);
    let verifying_key = signing_key.verifying_key();

    // Extract raw bytes
    let private_key_bytes = signing_key.to_bytes();
    let public_key_bytes = verifying_key.to_bytes();

    // Encode as base64
    let private_key_b64 = general_purpose::STANDARD.encode(&private_key_bytes);
    let public_key_b64 = general_purpose::STANDARD.encode(&public_key_bytes);

    let key_id = "ed25519:auto".to_string();
    let created_at = chrono::Utc::now();
    let expires_at = created_at + chrono::Duration::days(365); // 1 year validity

    // Store in database
    let query = "
        CREATE server_signing_keys SET
            server_name = $server_name,
            key_id = $key_id,
            public_key = $public_key,
            private_key = $private_key,
            created_at = $created_at,
            expires_at = $expires_at,
            is_active = true
    ";

    state
        .db
        .query(query)
        .bind(("server_name", server_name.to_string()))
        .bind(("key_id", key_id.clone()))
        .bind(("public_key", public_key_b64.clone()))
        .bind(("private_key", private_key_b64.clone()))
        .bind(("created_at", created_at))
        .bind(("expires_at", expires_at))
        .await?;

    info!(
        "Generated and stored new Ed25519 keypair for server {} with key_id {}",
        server_name, key_id
    );

    Ok(SigningKeyRecord {
        key_id,
        public_key: public_key_b64,
        private_key: private_key_b64,
        created_at,
        expires_at: Some(expires_at),
    })
}

/// Build canonical JSON for server key signing
fn build_canonical_server_json(
    server_name: &str,
    verify_keys: &serde_json::Map<String, Value>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let current_time_ms = chrono::Utc::now().timestamp_millis();
    let valid_until_ms = current_time_ms + (7 * 24 * 60 * 60 * 1000);

    let server_object = json!({
        "server_name": server_name,
        "verify_keys": verify_keys,
        "old_verify_keys": {},
        "valid_until_ts": valid_until_ms
    });

    Ok(to_canonical_json(&server_object)?)
}

/// Sign canonical JSON with Ed25519 private key
fn sign_canonical_json(
    canonical_json: &str,
    private_key_b64: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    use ed25519_dalek::{Signer, SigningKey};

    // Decode the base64 private key
    let private_key_bytes = general_purpose::STANDARD.decode(private_key_b64)?;

    // Validate private key length
    if private_key_bytes.len() != 32 {
        return Err("Invalid private key length".into());
    }

    // Convert to array and create SigningKey
    let private_key_array: [u8; 32] = private_key_bytes
        .try_into()
        .map_err(|_| "Failed to convert private key bytes to array")?;
    let signing_key = SigningKey::from_bytes(&private_key_array);

    // Sign the canonical JSON
    let signature = signing_key.sign(canonical_json.as_bytes());

    // Encode signature as base64
    let signature_b64 = general_purpose::STANDARD.encode(signature.to_bytes());

    Ok(signature_b64)
}
