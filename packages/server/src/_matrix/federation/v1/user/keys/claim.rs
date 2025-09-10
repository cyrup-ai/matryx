use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, error};

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ClaimRequest {
    one_time_keys: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyObject {
    key: String,
    signatures: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
pub struct ClaimResponse {
    one_time_keys: std::collections::HashMap<
        String,
        std::collections::HashMap<String, std::collections::HashMap<String, KeyObject>>,
    >,
}

/// POST /_matrix/federation/v1/user/keys/claim
///
/// Claims one-time keys for use in pre-key messages.
pub async fn post(
    State(state): State<AppState>,
    Json(payload): Json<ClaimRequest>,
) -> Result<Json<ClaimResponse>, StatusCode> {
    debug!("Federation keys claim request: {:?}", payload);

    let mut response_keys = std::collections::HashMap::new();

    for (user_id, device_requests) in payload.one_time_keys {
        debug!("Processing key claims for user: {}", user_id);

        let mut user_keys = std::collections::HashMap::new();

        for (device_id, algorithm) in device_requests {
            debug!("Claiming {} key for device: {}", algorithm, device_id);

            // Query device from database
            let query = "
                SELECT device_keys, one_time_keys, fallback_keys 
                FROM device 
                WHERE user_id = $user_id AND device_id = $device_id
            ";

            let mut result = state
                .db
                .query(query)
                .bind(("user_id", user_id.clone()))
                .bind(("device_id", device_id.clone()))
                .await
                .map_err(|e| {
                    error!("Database query failed: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            if let Ok(devices) = result.take::<Vec<serde_json::Value>>(0) {
                if let Some(device) = devices.first().cloned() {
                    let mut device_keys = std::collections::HashMap::new();

                    // Try to claim one-time key first
                    if let Some(otks_value) = device.get("one_time_keys").cloned() {
                        if let Some(otks) = otks_value.as_object().cloned() {
                            for (key_id, key_data) in otks {
                                if key_id.starts_with(&algorithm) {
                                    if let Ok(key_obj) =
                                        serde_json::from_value::<KeyObject>(key_data.clone())
                                    {
                                        device_keys.insert(key_id.clone(), key_obj);

                                        // Remove claimed key from database (ensure single use)
                                        let update_query = "
                                        UPDATE device SET one_time_keys -= $key_id 
                                        WHERE user_id = $user_id AND device_id = $device_id
                                    ";

                                        if let Err(e) = state
                                            .db
                                            .query(update_query)
                                            .bind(("key_id", key_id))
                                            .bind(("user_id", user_id.clone()))
                                            .bind(("device_id", device_id.clone()))
                                            .await
                                        {
                                            error!("Failed to remove claimed key: {}", e);
                                        }

                                        break; // Only claim one key per request
                                    }
                                }
                            }
                        }
                    }

                    // If no one-time key available, try fallback key
                    if device_keys.is_empty() {
                        if let Some(fbks_value) = device.get("fallback_keys").cloned() {
                            if let Some(fbks) = fbks_value.as_object() {
                                for (key_id, key_data) in fbks {
                                    if key_id.starts_with(&algorithm) {
                                        if let Ok(key_obj) =
                                            serde_json::from_value::<KeyObject>(key_data.clone())
                                        {
                                            device_keys.insert(key_id.clone(), key_obj);
                                            break; // Only one fallback key
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if !device_keys.is_empty() {
                        user_keys.insert(device_id, device_keys);
                    }
                }
            }
        }

        if !user_keys.is_empty() {
            response_keys.insert(user_id, user_keys);
        }
    }

    debug!("Federation keys claim response prepared for {} users", response_keys.len());

    Ok(Json(ClaimResponse { one_time_keys: response_keys }))
}
