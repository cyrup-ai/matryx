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
    crypto::MatryxCryptoProvider,
};
use matryx_surrealdb::repository::KeysRepository;

#[derive(Deserialize)]
pub struct KeysUploadRequest {
    pub device_keys: Option<DeviceKeys>,
    pub one_time_keys: Option<std::collections::HashMap<String, Value>>,
    pub fallback_keys: Option<std::collections::HashMap<String, Value>>,
}

// Use DeviceKeys from entity package to avoid type conflicts
use matryx_entity::types::DeviceKeys;

async fn store_device_keys(
    keys_repo: &KeysRepository,
    user_id: &str,
    device_id: &str,
    device_keys: &DeviceKeys,
) -> Result<(), StatusCode> {
    // Pass the entity DeviceKeys directly - no conversion needed
    keys_repo
        .store_device_keys(user_id, device_id, device_keys)
        .await
        .map_err(|e| {
            error!("Failed to store device keys: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(())
}

async fn store_one_time_keys(
    keys_repo: &KeysRepository,
    user_id: &str,
    device_id: &str,
    one_time_keys: &std::collections::HashMap<String, Value>,
) -> Result<(), StatusCode> {
    keys_repo
        .store_one_time_keys(user_id, device_id, one_time_keys)
        .await
        .map_err(|e| {
            error!("Failed to store one-time keys: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(())
}

async fn get_one_time_key_counts(
    keys_repo: &KeysRepository,
    user_id: &str,
    device_id: &str,
) -> Result<std::collections::HashMap<String, u32>, StatusCode> {
    keys_repo.get_one_time_key_counts(user_id, device_id).await.map_err(|e| {
        error!("Failed to get one-time key counts: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })
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
    let keys_repo = KeysRepository::new(state.db.clone());

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
        store_device_keys(&keys_repo, &user_id, &device_id, device_keys)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        info!("Device keys uploaded and validated for user: {} device: {}", user_id, device_id);

        // Generate device list update with Matrix-compliant sequential stream ID
        use crate::federation::device_management::DeviceListUpdate;

        // Generate sequential stream_id using SurrealDB SEQUENCE per user
        let stream_id = {
            let seq_name = format!("device_stream_{}", user_id.replace(':', "_"));
            state.db
                .query(format!("DEFINE SEQUENCE IF NOT EXISTS {} START 1;", seq_name))
                .await
                .map_err(|e| {
                    error!("Failed to define device stream sequence: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            let mut response = state.db
                .query(format!("RETURN sequence::nextval('{}');", seq_name))
                .await
                .map_err(|e| {
                    error!("Failed to get next stream_id: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            let stream_id_opt: Option<i64> = response.take(0).map_err(|e| {
                error!("Failed to extract stream_id from sequence::nextval: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

            stream_id_opt.ok_or_else(|| {
                error!("sequence::nextval returned None - sequence may not exist");
                StatusCode::INTERNAL_SERVER_ERROR
            })?
        };

        let device_update = DeviceListUpdate {
            user_id: user_id.clone(),
            device_id: device_id.clone(),
            stream_id,
            prev_id: vec![],
            deleted: false,
            device_display_name: None,
            keys: None,
        };

        if let Err(e) = state.device_manager.apply_device_update(&device_update).await {
            error!("Failed to apply device update: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
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

            // Track key counts by algorithm
            let algorithm = key_id.split(':').next().unwrap_or("unknown");
            *one_time_key_counts.entry(algorithm.to_string()).or_insert(0) += 1;
        }

        store_one_time_keys(&keys_repo, &user_id, &device_id, one_time_keys)
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
        keys_repo
            .store_fallback_keys(&user_id, &device_id, fallback_keys)
            .await
            .map_err(|e| {
                error!("Failed to store fallback keys: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!(
            "Fallback keys uploaded: {} keys for user: {} device: {}",
            fallback_keys.len(),
            user_id,
            device_id
        );
    }

    // Get current counts including newly uploaded keys
    let mut counts = get_one_time_key_counts(&keys_repo, &user_id, &device_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Merge newly uploaded key counts
    for (algorithm, count) in one_time_key_counts {
        *counts.entry(algorithm).or_insert(0) += count;
    }

    Ok(Json(json!({
        "one_time_key_counts": counts
    })))
}
