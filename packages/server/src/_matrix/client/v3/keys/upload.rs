use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
    crypto::MatryxCryptoProvider,
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

async fn store_device_keys(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
    device_id: &str,
    device_keys: &DeviceKeys,
) -> Result<(), StatusCode> {
    let _: Option<DeviceKeys> = db
        .create(("device_keys", format!("{}:{}", user_id, device_id)))
        .content(json!({
            "user_id": user_id,
            "device_id": device_id,
            "device_keys": device_keys,
            "created_at": Utc::now(),
            "signature_valid": true,
            "validation_timestamp": Utc::now()
        }))
        .await
        .map_err(|e| {
            error!("Failed to store device keys: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(())
}

async fn store_one_time_keys(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
    device_id: &str,
    one_time_keys: &std::collections::HashMap<String, Value>,
) -> Result<(), StatusCode> {
    for (key_id, key_data) in one_time_keys {
        let _: Option<Value> = db
            .create(("one_time_keys", format!("{}:{}:{}", user_id, device_id, key_id)))
            .content(json!({
                "key_id": key_id,
                "key": key_data,
                "user_id": user_id,
                "device_id": device_id,
                "created_at": Utc::now(),
                "claimed": false,
                "algorithm_type": key_id.split(':').next().unwrap_or("unknown"),
                "vodozemac_validated": true
            }))
            .await
            .map_err(|e| {
                error!("Failed to store one-time key: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
    }

    Ok(())
}

async fn get_one_time_key_counts(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
    device_id: &str,
) -> Result<std::collections::HashMap<String, u32>, StatusCode> {
    let query = "SELECT algorithm_type, count() AS count FROM one_time_keys WHERE user_id = $user_id AND device_id = $device_id AND claimed = false GROUP BY algorithm_type";
    let user_id_owned = user_id.to_string();
    let device_id_owned = device_id.to_string();
    let mut response = db
        .query(query)
        .bind(("user_id", user_id_owned))
        .bind(("device_id", device_id_owned))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results: Vec<Value> = response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut counts = std::collections::HashMap::new();
    for result in results {
        if let (Some(algorithm), Some(count)) = (
            result.get("algorithm_type").and_then(|v| v.as_str()),
            result.get("count").and_then(|v| v.as_u64()),
        ) {
            counts.insert(algorithm.to_string(), count as u32);
        }
    }

    Ok(counts)
}

/// POST /_matrix/client/v3/keys/upload
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<KeysUploadRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
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

    let crypto_provider = MatryxCryptoProvider::new(state.db.clone());

    // Enhanced device key validation with vodozemac
    if let Some(device_keys) = &request.device_keys {
        // Validate device keys match authenticated device
        if device_keys.user_id != user_id || device_keys.device_id != device_id {
            return Err(StatusCode::FORBIDDEN);
        }

        // Convert to entity type for validation
        let entity_device_keys = matryx_entity::types::DeviceKey {
            user_id: device_keys.user_id.clone(),
            device_id: device_keys.device_id.clone(),
            algorithms: device_keys.algorithms.clone(),
            keys: device_keys.keys.clone(),
            signatures: device_keys.signatures.clone(),
            unsigned: None,
        };

        // NEW: Cryptographic validation using vodozemac
        if !crypto_provider
            .verify_device_keys(&entity_device_keys)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?
        {
            error!(
                "Device key signature validation failed for user: {} device: {}",
                user_id, device_id
            );
            return Err(StatusCode::BAD_REQUEST);
        }

        // Store validated keys
        store_device_keys(&state.db, &user_id, &device_id, device_keys)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        info!("Device keys uploaded and validated for user: {} device: {}", user_id, device_id);
    }

    let mut one_time_key_counts: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();

    // Enhanced one-time key validation
    if let Some(one_time_keys) = &request.one_time_keys {
        for (key_id, key_data) in one_time_keys {
            // NEW: Validate key format using vodozemac
            if !crypto_provider
                .validate_one_time_key(key_id, key_data)
                .await
                .map_err(|_| StatusCode::BAD_REQUEST)?
            {
                error!("Invalid one-time key format: {}", key_id);
                return Err(StatusCode::BAD_REQUEST);
            }
        }

        store_one_time_keys(&state.db, &user_id, &device_id, one_time_keys)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        info!(
            "One-time keys uploaded and validated: {} keys for user: {} device: {}",
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

    // Get current counts
    let counts = get_one_time_key_counts(&state.db, &user_id, &device_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "one_time_key_counts": counts
    })))
}
