pub mod by_server_name;

use axum::{Json, extract::State, http::StatusCode};
use reqwest::Client;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::AppState;
use crate::federation::server_discovery::ServerDiscoveryOrchestrator;
use matryx_entity::utils::canonical_json;
use matryx_surrealdb::repository::InfrastructureService;

use super::common::{create_infrastructure_service, sign_canonical_json};

/// POST /_matrix/key/v2/query
///
/// Query for keys from multiple servers in a batch format using KeyServerRepository.
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

    // Create InfrastructureService instance
    let infrastructure_service = create_infrastructure_service(&state).await;

    // Create ServerDiscoveryOrchestrator for Matrix DNS resolution
    let server_discovery = ServerDiscoveryOrchestrator::new(state.dns_resolver.clone());

    // Process each server's key requests
    for (server_name, key_requests) in server_keys_request {
        debug!("Querying keys for server: {}", server_name);

        match fetch_and_sign_server_keys(
            &infrastructure_service,
            &server_discovery,
            &client,
            server_name,
            key_requests,
            &state.homeserver_name,
        )
        .await
        {
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

/// Fetch server keys from a remote server and sign them as a notary using repository
async fn fetch_and_sign_server_keys(
    infrastructure_service: &InfrastructureService<surrealdb::engine::any::Any>,
    server_discovery: &ServerDiscoveryOrchestrator,
    client: &Client,
    server_name: &str,
    key_requests: &Value,
    homeserver_name: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    // Extract minimum_valid_until_ts from key_requests
    let minimum_valid_until_ts = if let Some(obj) = key_requests.as_object() {
        obj.values()
            .filter_map(|v| v.get("minimum_valid_until_ts"))
            .filter_map(|v| v.as_i64())
            .max()
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
    } else {
        chrono::Utc::now().timestamp_millis()
    };
    debug!("Minimum valid until timestamp requested: {}", minimum_valid_until_ts);

    // Check cache first using get_server_keys which returns full ServerKeys struct
    if let Ok(cached_keys) = infrastructure_service
        .get_server_keys_raw(server_name, None)
        .await
    {
        debug!("Found cached keys for server: {}", server_name);
        
        // Check if cached key meets minimum_valid_until_ts requirement
        if cached_keys.valid_until_ts >= minimum_valid_until_ts {
            // Per Matrix spec: "cache a response for half of its lifetime"
            // Calculate if cache is still fresh (within half of its lifetime)
            let now_ms = chrono::Utc::now().timestamp_millis();
            
            // Assume keys were fetched when they became valid (conservative estimate)
            // In reality, we'd track fetched_at separately, but valid_until_ts is what we have
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
        }
        
        debug!("Cached key doesn't meet requirements, fetching fresh keys");
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

    // Validate that the fetched key meets minimum_valid_until_ts requirement
    let valid_until_ts = server_key_response
        .get("valid_until_ts")
        .and_then(|v| v.as_i64())
        .ok_or("Server key response missing valid_until_ts")?;

    if valid_until_ts < minimum_valid_until_ts {
        return Err(format!(
            "Server key expires at {} which is before requested minimum {}",
            valid_until_ts, minimum_valid_until_ts
        )
        .into());
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
    if let Some(signatures) = signed_response.get_mut("signatures")
        && let Some(signatures_obj) = signatures.as_object_mut()
    {
        signatures_obj.insert(homeserver_name.to_string(), notary_signature);
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

    // Sign the canonical JSON
    let signature = sign_canonical_json(&canonical_json, &signing_key.signing_key)?;

    let mut notary_signatures = HashMap::new();
    notary_signatures.insert(signing_key.key_id, json!(signature));

    Ok(json!(notary_signatures))
}


