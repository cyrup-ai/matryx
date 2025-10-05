use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{error, info};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_surrealdb::repository::KeysRepository;

#[derive(Deserialize)]
pub struct KeysClaimRequest {
    pub one_time_keys: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    pub timeout: Option<u64>,
}

/// POST /_matrix/client/v3/keys/claim
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<KeysClaimRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        error!("Keys claim failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let _user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        _ => return Err(StatusCode::FORBIDDEN),
    };

    let mut claimed_keys = std::collections::HashMap::new();
    let mut failures = std::collections::HashMap::new();
    let keys_repo = KeysRepository::new(state.db.clone());

    // Handle timeout parameter from request (Matrix spec: max time client will wait)
    let timeout_ms = request.timeout.unwrap_or(10000); // Default 10 seconds
    if timeout_ms > 0 {
        info!("Keys claim request with timeout: {}ms", timeout_ms);
    }

    // Process each user's device key claims
    for (user_id, device_algorithms) in &request.one_time_keys {
        let mut user_claimed_keys = std::collections::HashMap::new();

        for (device_id, algorithm) in device_algorithms {
            // Try to claim a one-time key first
            match keys_repo.claim_one_time_keys(user_id, device_id, algorithm).await {
                Ok(Some((key_id, key_value))) => {
                    // Successfully claimed a one-time key
                    let mut device_keys = std::collections::HashMap::new();
                    device_keys.insert(key_id.clone(), key_value);
                    user_claimed_keys.insert(device_id.clone(), device_keys);

                    info!(
                        "One-time key claimed: user={} device={} algorithm={} key_id={}",
                        user_id, device_id, algorithm, key_id
                    );
                },
                Ok(None) => {
                    // No one-time key available, try fallback keys
                    match keys_repo.find_fallback_keys(user_id, device_id, algorithm).await {
                        Ok(Some((key_id, key_value))) => {
                            let mut device_keys = std::collections::HashMap::new();
                            device_keys.insert(key_id.clone(), key_value);
                            user_claimed_keys.insert(device_id.clone(), device_keys);

                            info!(
                                "Fallback key used: user={} device={} algorithm={} key_id={}",
                                user_id, device_id, algorithm, key_id
                            );
                        },
                        Ok(None) => {
                            // No keys available
                            failures.insert(
                                format!("{}:{}", user_id, device_id),
                                json!({
                                    "error": "No one-time keys available"
                                }),
                            );
                        },
                        Err(e) => {
                            error!("Failed to find fallback keys: {}", e);
                            failures.insert(
                                format!("{}:{}", user_id, device_id),
                                json!({
                                    "error": "Failed to query fallback keys"
                                }),
                            );
                        },
                    }
                },
                Err(e) => {
                    error!(
                        "Failed to claim one-time keys for user {} device {}: {}",
                        user_id, device_id, e
                    );
                    failures.insert(
                        format!("{}:{}", user_id, device_id),
                        json!({
                            "error": "Failed to claim one-time keys"
                        }),
                    );
                },
            }
        }

        if !user_claimed_keys.is_empty() {
            claimed_keys.insert(user_id.clone(), user_claimed_keys);
        }
    }

    info!("Keys claim completed for {} users", request.one_time_keys.len());

    Ok(Json(json!({
        "one_time_keys": claimed_keys,
        "failures": failures
    })))
}
