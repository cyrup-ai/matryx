use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};

#[derive(Deserialize)]
pub struct KeysQueryRequest {
    pub device_keys: std::collections::HashMap<String, Vec<String>>,
    pub timeout: Option<u64>,
    pub token: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct DeviceKeys {
    pub user_id: String,
    pub device_id: String,
    pub algorithms: Vec<String>,
    pub keys: std::collections::HashMap<String, String>,
    pub signatures: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

/// POST /_matrix/client/v3/keys/query
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<KeysQueryRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        error!("Keys query failed - authentication extraction failed: {}", e);
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

    let mut device_keys: std::collections::HashMap<
        String,
        std::collections::HashMap<String, Value>,
    > = std::collections::HashMap::new();
    let mut failures: std::collections::HashMap<String, Value> = std::collections::HashMap::new();
    let mut master_keys: std::collections::HashMap<String, Value> =
        std::collections::HashMap::new();
    let mut self_signing_keys: std::collections::HashMap<String, Value> =
        std::collections::HashMap::new();
    let mut user_signing_keys: std::collections::HashMap<String, Value> =
        std::collections::HashMap::new();

    // Query device keys for each requested user
    for (user_id, device_ids) in &request.device_keys {
        let mut user_device_keys = std::collections::HashMap::new();

        if device_ids.is_empty() {
            // Query all devices for this user
            let query = "SELECT * FROM device_keys WHERE user_id = $user_id";
            let mut response = state
                .db
                .query(query)
                .bind(("user_id", user_id.clone()))
                .await
                .map_err(|e| {
                    error!("Failed to query device keys for user {}: {}", user_id, e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            let devices: Vec<Value> =
                response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            for device in devices {
                if let Some(device_data) = device.get("device_keys") {
                    if let Some(device_id) = device_data.get("device_id").and_then(|v| v.as_str()) {
                        user_device_keys.insert(device_id.to_string(), device_data.clone());
                    }
                }
            }
        } else {
            // Query specific devices
            for device_id in device_ids {
                let query =
                    "SELECT * FROM device_keys WHERE user_id = $user_id AND device_id = $device_id";
                let mut response = state
                    .db
                    .query(query)
                    .bind(("user_id", user_id.clone()))
                    .bind(("device_id", device_id.clone()))
                    .await
                    .map_err(|e| {
                        error!(
                            "Failed to query device key for user {} device {}: {}",
                            user_id, device_id, e
                        );
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?;

                let device: Option<Value> =
                    response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                if let Some(device_data) = device {
                    if let Some(keys) = device_data.get("device_keys") {
                        user_device_keys.insert(device_id.clone(), keys.clone());
                    }
                } else {
                    failures.insert(
                        format!("{}:{}", user_id, device_id),
                        json!({
                            "error": "Device not found"
                        }),
                    );
                }
            }
        }

        if !user_device_keys.is_empty() {
            device_keys.insert(user_id.clone(), user_device_keys);
        }

        // Query cross-signing keys for this user
        let cross_signing_query = "SELECT * FROM cross_signing_keys WHERE user_id = $user_id";
        let mut cross_signing_response = state
            .db
            .query(cross_signing_query)
            .bind(("user_id", user_id.clone()))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let cross_signing_keys: Vec<Value> = cross_signing_response
            .take(0)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        for key in cross_signing_keys {
            if let Some(key_type) = key.get("key_type").and_then(|v| v.as_str()) {
                if let Some(key_data) = key.get("key_data") {
                    match key_type {
                        "master" => {
                            master_keys.insert(user_id.clone(), key_data.clone());
                        },
                        "self_signing" => {
                            self_signing_keys.insert(user_id.clone(), key_data.clone());
                        },
                        "user_signing" => {
                            user_signing_keys.insert(user_id.clone(), key_data.clone());
                        },
                        _ => {},
                    }
                }
            }
        }
    }

    info!("Keys query completed for {} users", request.device_keys.len());

    Ok(Json(json!({
        "device_keys": device_keys,
        "failures": failures,
        "master_keys": master_keys,
        "self_signing_keys": self_signing_keys,
        "user_signing_keys": user_signing_keys
    })))
}
