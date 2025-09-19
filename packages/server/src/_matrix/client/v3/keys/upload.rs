use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};

#[derive(Deserialize)]
pub struct KeysUploadRequest {
    pub device_keys: Option<DeviceKeys>,
    pub one_time_keys: Option<std::collections::HashMap<String, Value>>,
    pub fallback_keys: Option<std::collections::HashMap<String, Value>>,
}

#[derive(Serialize, Deserialize)]
pub struct DeviceKeys {
    pub user_id: String,
    pub device_id: String,
    pub algorithms: Vec<String>,
    pub keys: std::collections::HashMap<String, String>,
    pub signatures: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

fn validate_device_keys_signature(
    device_keys: &DeviceKeys,
    user_id: &str,
    device_id: &str,
) -> Result<(), StatusCode> {
    // Validate user_id and device_id match
    if device_keys.user_id != user_id || device_keys.device_id != device_id {
        error!("Device keys user_id or device_id mismatch");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate required algorithms
    if !device_keys
        .algorithms
        .contains(&"m.olm.v1.curve25519-aes-sha2".to_string())
    {
        error!("Device keys missing required algorithm");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate keys exist
    if device_keys.keys.is_empty() {
        error!("Device keys missing keys");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate signature exists
    if !device_keys.signatures.contains_key(user_id) {
        error!("Device keys missing user signature");
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(())
}

/// POST /_matrix/client/v3/keys/upload
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<KeysUploadRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        error!("Keys upload failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let (user_id, device_id) = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            (token_info.user_id.clone(), token_info.device_id.clone())
        },
        _ => return Err(StatusCode::FORBIDDEN),
    };

    // Validate and store device keys
    if let Some(device_keys) = &request.device_keys {
        validate_device_keys_signature(device_keys, &user_id, &device_id)?;

        let _: Option<DeviceKeys> = state
            .db
            .create(("device_keys", format!("{}:{}", user_id, device_id)))
            .content(json!({
                "user_id": user_id,
                "device_id": device_id,
                "device_keys": device_keys,
                "created_at": Utc::now()
            }))
            .await
            .map_err(|e| {
                error!("Failed to store device keys: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!("Device keys uploaded for user: {} device: {}", user_id, device_id);
    }

    let mut one_time_key_counts = std::collections::HashMap::new();

    // Store one-time keys
    if let Some(one_time_keys) = &request.one_time_keys {
        for (key_id, key_data) in one_time_keys {
            let _: Option<Value> = state
                .db
                .create(("one_time_keys", format!("{}:{}:{}", user_id, device_id, key_id)))
                .content(json!({
                    "key_id": key_id,
                    "key": key_data,
                    "user_id": user_id,
                    "device_id": device_id,
                    "created_at": Utc::now(),
                    "claimed": false
                }))
                .await
                .map_err(|e| {
                    error!("Failed to store one-time key: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            // Count keys by algorithm
            let algorithm = key_id.split(':').next().unwrap_or("unknown");
            *one_time_key_counts.entry(algorithm.to_string()).or_insert(0) += 1;
        }

        info!(
            "One-time keys uploaded: {} keys for user: {} device: {}",
            one_time_keys.len(),
            user_id,
            device_id
        );
    }

    // Store fallback keys
    if let Some(fallback_keys) = &request.fallback_keys {
        for (key_id, key_data) in fallback_keys {
            let _: Option<Value> = state
                .db
                .create(("fallback_keys", format!("{}:{}:{}", user_id, device_id, key_id)))
                .content(json!({
                    "key_id": key_id,
                    "key": key_data,
                    "user_id": user_id,
                    "device_id": device_id,
                    "created_at": Utc::now()
                }))
                .await
                .map_err(|e| {
                    error!("Failed to store fallback key: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
        }

        info!(
            "Fallback keys uploaded: {} keys for user: {} device: {}",
            fallback_keys.len(),
            user_id,
            device_id
        );
    }

    Ok(Json(json!({
        "one_time_key_counts": one_time_key_counts
    })))
}
