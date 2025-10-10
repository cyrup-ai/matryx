use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use reqwest::Client;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::AppState;
use crate::federation::server_discovery::ServerDiscoveryOrchestrator;
use matryx_entity::utils::canonical_json;
use matryx_surrealdb::repository::InfrastructureService;

use super::super::common::{create_infrastructure_service, sign_canonical_json};

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

    // Create ServerDiscoveryOrchestrator for Matrix DNS resolution
    let server_discovery = ServerDiscoveryOrchestrator::new(state.dns_resolver.clone());

    // Fetch and sign server keys using repository
    match fetch_server_keys(
        &infrastructure_service,
        &server_discovery,
        &client,
        &server_name,
        &state.homeserver_name,
    )
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

/// Fetch server keys from a specific server and sign them as a notary using repository
async fn fetch_server_keys(
    infrastructure_service: &InfrastructureService<surrealdb::engine::any::Any>,
    server_discovery: &ServerDiscoveryOrchestrator,
    client: &Client,
    server_name: &str,
    homeserver_name: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    // Check cache first using get_server_keys which returns full ServerKeys struct
    if let Ok(cached_keys) = infrastructure_service
        .get_server_keys_raw(server_name, None)
        .await
    {
        debug!("Found cached keys for server: {}", server_name);
        
        // Per Matrix spec: "cache a response for half of its lifetime"
        // Calculate if cache is still fresh (within half of its lifetime)
        let now_ms = chrono::Utc::now().timestamp_millis();
        
        // Assume keys were fetched when they became valid (conservative estimate)
        let cache_lifetime_half = (cached_keys.valid_until_ts - now_ms) / 2;
        
        // If we're still within the valid period and have at least half the lifetime remaining
        if cached_keys.valid_until_ts > now_ms && cache_lifetime_half > 0 {
            debug!("Serving cached keys for server: {}", server_name);
            
            // Convert cached ServerKeys to JSON response format
            let mut verify_keys_map = serde_json::Map::new();
            for (key_id, verify_key) in &cached_keys.verify_keys {
                verify_keys_map.insert(
                    key_id.clone(),
                    json!({"key": verify_key.key}),
                );
            }
            
            let mut old_verify_keys_map = serde_json::Map::new();
            for (key_id, old_key) in &cached_keys.old_verify_keys {
                old_verify_keys_map.insert(
                    key_id.clone(),
                    json!({"key": old_key.key, "expired_ts": old_key.expired_ts}),
                );
            }
            
            let cached_response = json!({
                "server_name": cached_keys.server_name,
                "valid_until_ts": cached_keys.valid_until_ts,
                "verify_keys": verify_keys_map,
                "old_verify_keys": old_verify_keys_map,
                "signatures": cached_keys.signatures,
            });
            
            return Ok(vec![cached_response]);
        }
        
        debug!("Cached key expired or stale, fetching fresh keys");
    }

    // Resolve server using Matrix DNS resolution
    let connection = server_discovery.discover_server(server_name).await?;
    let url = format!("{}/_matrix/key/v2/server", connection.base_url);
    debug!("Fetching server keys from: {}", url);

    let response = client
        .get(&url)
        .header("User-Agent", "matryx-homeserver/1.0")
        .header("Host", connection.host_header)
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
        .ok_or("Server key response missing valid_until_ts")?;

    let current_time_ms = chrono::Utc::now().timestamp_millis();
    if valid_until_ts > 0 && current_time_ms > valid_until_ts {
        return Err(format!("Server key response has expired for {}", server_name).into());
    }

    // Cache the fetched keys for future requests
    // Convert the JSON response to ServerKeys struct for caching
    let verify_keys_value = server_key_response
        .get("verify_keys")
        .and_then(|v| v.as_object())
        .ok_or("Server key response missing verify_keys")?;
    
    let mut verify_keys = HashMap::new();
    for (key_id, key_data) in verify_keys_value {
        if let Some(key_str) = key_data.get("key").and_then(|v| v.as_str()) {
            verify_keys.insert(
                key_id.clone(),
                matryx_surrealdb::repository::VerifyKey {
                    key: key_str.to_string(),
                },
            );
        }
    }

    let empty_map = serde_json::Map::new();
    let old_verify_keys_value = server_key_response
        .get("old_verify_keys")
        .and_then(|v| v.as_object())
        .unwrap_or(&empty_map);
    
    let mut old_verify_keys = HashMap::new();
    for (key_id, key_data) in old_verify_keys_value {
        if let (Some(key_str), Some(expired_ts)) = (
            key_data.get("key").and_then(|v| v.as_str()),
            key_data.get("expired_ts").and_then(|v| v.as_i64()),
        ) {
            old_verify_keys.insert(
                key_id.clone(),
                matryx_surrealdb::repository::OldVerifyKey {
                    key: key_str.to_string(),
                    expired_ts,
                },
            );
        }
    }

    let signatures_value = server_key_response
        .get("signatures")
        .and_then(|v| v.as_object())
        .ok_or("Server key response missing signatures")?;
    
    let mut signatures = HashMap::new();
    for (server, sigs) in signatures_value {
        if let Some(sig_obj) = sigs.as_object() {
            let mut server_sigs = HashMap::new();
            for (key_id, sig) in sig_obj {
                if let Some(sig_str) = sig.as_str() {
                    server_sigs.insert(key_id.clone(), sig_str.to_string());
                }
            }
            signatures.insert(server.clone(), server_sigs);
        }
    }

    let server_keys = matryx_surrealdb::repository::ServerKeys {
        server_name: server_name.to_string(),
        valid_until_ts,
        verify_keys,
        old_verify_keys,
        signatures,
    };

    let valid_until = chrono::DateTime::from_timestamp_millis(valid_until_ts)
        .ok_or("Invalid valid_until_ts timestamp")?;

    // Store in cache
    if let Err(e) = infrastructure_service
        .store_server_keys(server_name, &server_keys, valid_until)
        .await
    {
        warn!("Failed to cache server keys for {}: {:?}", server_name, e);
        // Continue even if caching fails - not a critical error
    } else {
        debug!("Cached server keys for {} until {}", server_name, valid_until);
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


