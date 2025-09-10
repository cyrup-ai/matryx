pub mod by_server_name;

use axum::{Json, extract::State, http::StatusCode};
use reqwest::Client;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::AppState;

/// POST /_matrix/key/v2/query
///
/// Query for keys from multiple servers in a batch format.
/// The receiving (notary) server must sign the keys returned by the queried servers.
pub async fn post(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    info!("Processing batch server key query request");

    // Parse the server_keys request format
    let server_keys_request =
        payload.get("server_keys").and_then(|v| v.as_object()).ok_or_else(|| {
            warn!("Invalid server_keys format in batch query");
            StatusCode::BAD_REQUEST
        })?;

    let mut server_keys_response = Vec::new();

    // Create HTTP client for fetching remote keys
    let client = Client::builder().timeout(Duration::from_secs(30)).build().map_err(|e| {
        error!("Failed to create HTTP client for key queries: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Process each server's key requests
    for (server_name, key_requests) in server_keys_request {
        debug!("Querying keys for server: {}", server_name);

        match fetch_and_sign_server_keys(&state, &client, server_name, key_requests).await {
            Ok(signed_keys) => {
                server_keys_response.extend(signed_keys);
            },
            Err(e) => {
                warn!("Failed to fetch keys for server {}: {}", server_name, e);
                // Continue with other servers even if one fails
            },
        }
    }

    info!("Batch key query completed, returning {} server keys", server_keys_response.len());

    Ok(Json(json!({
        "server_keys": server_keys_response
    })))
}

/// Fetch server keys from a remote server and sign them as a notary
async fn fetch_and_sign_server_keys(
    state: &AppState,
    client: &Client,
    server_name: &str,
    _key_requests: &Value,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    // Fetch keys from the remote server's /_matrix/key/v2/server endpoint
    let url = format!("https://{}/_matrix/key/v2/server", server_name);
    debug!("Fetching server keys from: {}", url);

    let response = client
        .get(&url)
        .header("User-Agent", "matryx-homeserver/1.0")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!(
            "Server key request failed with status: {} for {}",
            response.status(),
            server_name
        )
        .into());
    }

    let server_key_response: Value = response.json().await?;

    // Verify the response is for the correct server
    let response_server_name = server_key_response
        .get("server_name")
        .and_then(|v| v.as_str())
        .ok_or("Server key response missing server_name")?;

    if response_server_name != server_name {
        return Err(format!(
            "Server key response server name mismatch: expected {}, got {}",
            server_name, response_server_name
        )
        .into());
    }

    // Sign the server key response as a notary
    let notary_signature = create_notary_signature(state, &server_key_response).await?;

    // Add our notary signature to the response
    let mut signed_response = server_key_response;
    if let Some(signatures) = signed_response.get_mut("signatures") {
        if let Some(signatures_obj) = signatures.as_object_mut() {
            signatures_obj.insert(state.homeserver_name.clone(), notary_signature);
        }
    }

    debug!("Successfully fetched and signed keys for server: {}", server_name);
    Ok(vec![signed_response])
}

/// Create a notary signature for a server key response
async fn create_notary_signature(
    state: &AppState,
    server_key_response: &Value,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    // Get our server's signing key
    let query = "
        SELECT key_id, private_key
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
        key_id: String,
        private_key: String,
    }

    let signing_key: Option<SigningKeyRecord> = response.take(0)?;
    let signing_key = signing_key.ok_or("No signing key found for notary signature")?;

    // Create canonical JSON for signing (without signatures field)
    let mut canonical_data = server_key_response.clone();
    if let Some(obj) = canonical_data.as_object_mut() {
        obj.remove("signatures");
    }

    let canonical_json = serde_json::to_string(&canonical_data)?;

    // Sign the canonical JSON
    let signature = sign_canonical_json(&canonical_json, &signing_key.private_key)?;

    let mut notary_signatures = HashMap::new();
    notary_signatures.insert(signing_key.key_id, json!(signature));

    Ok(json!(notary_signatures))
}

/// Sign canonical JSON with Ed25519 private key (from existing server.rs patterns)
fn sign_canonical_json(
    canonical_json: &str,
    private_key_b64: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    use base64::{Engine, engine::general_purpose};
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
