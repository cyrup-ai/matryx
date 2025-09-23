use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};

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

    // Process each user's device key claims
    for (user_id, device_algorithms) in &request.one_time_keys {
        let mut user_claimed_keys = std::collections::HashMap::new();

        for (device_id, algorithm) in device_algorithms {
            // Find an available one-time key for this user/device/algorithm
            let query = "
                SELECT * FROM one_time_keys 
                WHERE user_id = $user_id 
                  AND device_id = $device_id 
                  AND key_id LIKE $algorithm_pattern
                  AND claimed = false 
                LIMIT 1
            ";

            let algorithm_pattern = format!("{}:%", algorithm);
            let mut response = state
                .db
                .query(query)
                .bind(("user_id", user_id.clone()))
                .bind(("device_id", device_id.clone()))
                .bind(("algorithm_pattern", algorithm_pattern.clone()))
                .await
                .map_err(|e| {
                    error!(
                        "Failed to query one-time keys for user {} device {}: {}",
                        user_id, device_id, e
                    );
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            let key_record: Option<Value> =
                response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            if let Some(key_data) = key_record {
                let key_id =
                    key_data.get("key_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let key_value = key_data.get("key").cloned().unwrap_or(json!({}));

                // Mark the key as claimed
                let update_query = "UPDATE one_time_keys SET claimed = true WHERE user_id = $user_id AND device_id = $device_id AND key_id = $key_id";
                let _update_result = state
                    .db
                    .query(update_query)
                    .bind(("user_id", user_id.clone()))
                    .bind(("device_id", device_id.clone()))
                    .bind(("key_id", key_id.clone()))
                    .await
                    .map_err(|e| {
                        error!("Failed to mark one-time key as claimed: {}", e);
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                // Add to claimed keys response
                let mut device_keys = std::collections::HashMap::new();
                device_keys.insert(key_id.clone(), key_value);
                user_claimed_keys.insert(device_id.clone(), device_keys);

                info!(
                    "One-time key claimed: user={} device={} algorithm={} key_id={}",
                    user_id, device_id, algorithm, key_id
                );
            } else {
                // No available key found - check for fallback keys
                let fallback_query = "
                    SELECT * FROM fallback_keys 
                    WHERE user_id = $user_id 
                      AND device_id = $device_id 
                      AND key_id LIKE $algorithm_pattern
                    LIMIT 1
                ";

                let mut fallback_response = state
                    .db
                    .query(fallback_query)
                    .bind(("user_id", user_id.clone()))
                    .bind(("device_id", device_id.clone()))
                    .bind(("algorithm_pattern", algorithm_pattern))
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                let fallback_key: Option<Value> =
                    fallback_response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                if let Some(fallback_data) = fallback_key {
                    let key_id = fallback_data
                        .get("key_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let key_value = fallback_data.get("key").cloned().unwrap_or(json!({}));

                    let mut device_keys = std::collections::HashMap::new();
                    device_keys.insert(key_id.clone(), key_value);
                    user_claimed_keys.insert(device_id.clone(), device_keys);

                    info!(
                        "Fallback key used: user={} device={} algorithm={} key_id={}",
                        user_id, device_id, algorithm, key_id
                    );
                } else {
                    // No keys available
                    failures.insert(
                        format!("{}:{}", user_id, device_id),
                        json!({
                            "error": "No one-time keys available"
                        }),
                    );
                }
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
