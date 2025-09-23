use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use reqwest::Client;
use serde_json::{Value, json};
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::AppState;
use matryx_entity::utils::canonical_json;
use matryx_surrealdb::repository::InfrastructureService;

/// GET /_matrix/key/v2/query/{serverName}
///
/// Query for another server's keys using KeyServerRepository.
/// The receiving (notary) server must sign the keys returned by the queried server.
pub async fn get(
    State(state): State<AppState>,
    Path(server_name): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    info!("Processing single server key query for: {}", server_name);

    // Validate server name format
    if server_name.is_empty() || !server_name.contains('.') {
        warn!("Invalid server name format: {}", server_name);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Create HTTP client for fetching remote keys
    let client = Client::builder().timeout(Duration::from_secs(30)).build().map_err(|e| {
        error!("Failed to create HTTP client for key query: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Create InfrastructureService instance
    let infrastructure_service = create_infrastructure_service(&state).await;

    // Fetch and sign server keys using repository
    match fetch_server_keys(&infrastructure_service, &client, &server_name, &state.homeserver_name)
        .await
    {
        Ok(server_keys) => {
            info!("Successfully fetched keys for server: {}", server_name);
            Ok(Json(json!({
                "server_keys": server_keys
            })))
        },
        Err(e) => {
            error!("Failed to fetch keys for server {}: {}", server_name, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
    }
}

async fn create_infrastructure_service(
    state: &AppState,
) -> InfrastructureService<surrealdb::engine::any::Any> {
    let websocket_repo = matryx_surrealdb::repository::WebSocketRepository::new(state.db.clone());
    let transaction_repo =
        matryx_surrealdb::repository::TransactionRepository::new(state.db.clone());
    let key_server_repo = matryx_surrealdb::repository::KeyServerRepository::new(state.db.clone());
    let registration_repo =
        matryx_surrealdb::repository::RegistrationRepository::new(state.db.clone());
    let directory_repo = matryx_surrealdb::repository::DirectoryRepository::new(state.db.clone());
    let device_repo = matryx_surrealdb::repository::DeviceRepository::new(state.db.clone());

    InfrastructureService::new(
        websocket_repo,
        transaction_repo,
        key_server_repo,
        registration_repo,
        directory_repo,
        device_repo,
    )
}

/// Fetch server keys from a specific server and sign them as a notary using repository
async fn fetch_server_keys(
    infrastructure_service: &InfrastructureService<surrealdb::engine::any::Any>,
    client: &Client,
    server_name: &str,
    homeserver_name: &str,
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

    // Verify the key response hasn't expired
    let valid_until_ts = server_key_response
        .get("valid_until_ts")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let current_time_ms = chrono::Utc::now().timestamp_millis();
    if valid_until_ts > 0 && current_time_ms > valid_until_ts {
        return Err(format!("Server key response has expired for {}", server_name).into());
    }

    // Sign the server key response as a notary using repository
    let notary_signature =
        create_notary_signature(infrastructure_service, &server_key_response, homeserver_name)
            .await?;

    // Add our notary signature to the response
    let mut signed_response = server_key_response;
    if let Some(signatures) = signed_response.get_mut("signatures") {
        if let Some(signatures_obj) = signatures.as_object_mut() {
            signatures_obj.insert(homeserver_name.to_string(), notary_signature);
        }
    } else {
        // Create signatures object if it doesn't exist
        let mut signatures = std::collections::HashMap::new();
        signatures.insert(homeserver_name.to_string(), notary_signature);
        signed_response
            .as_object_mut()
            .ok_or("Server key response is not an object")?
            .insert("signatures".to_string(), json!(signatures));
    }

    debug!("Successfully fetched and signed keys for server: {}", server_name);
    Ok(vec![signed_response])
}

/// Create a notary signature for a server key response using repository
async fn create_notary_signature(
    infrastructure_service: &InfrastructureService<surrealdb::engine::any::Any>,
    server_key_response: &Value,
    homeserver_name: &str,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    use std::collections::HashMap;

    // Get our server's signing key using repository
    let key_id = "ed25519:auto"; // Default key ID format
    let signing_key = infrastructure_service
        .get_signing_key(homeserver_name, key_id)
        .await
        .map_err(|e| format!("Failed to get signing key: {:?}", e))?;

    let signing_key = signing_key.ok_or("No signing key found for notary signature")?;

    // Create canonical JSON for signing (without signatures field)
    let mut canonical_data = server_key_response.clone();
    if let Some(obj) = canonical_data.as_object_mut() {
        obj.remove("signatures");
    }

    let canonical_json = canonical_json(&canonical_data)?;

    // Sign the canonical JSON using existing patterns
    let signature = sign_canonical_json(&canonical_json, &signing_key.signing_key)?;

    let mut notary_signatures = HashMap::new();
    notary_signatures.insert(signing_key.key_id, json!(signature));

    Ok(json!(notary_signatures))
}

/// Sign canonical JSON with Ed25519 private key
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
