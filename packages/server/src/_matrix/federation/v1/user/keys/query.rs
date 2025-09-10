use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, error};

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    device_keys: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct DeviceKeys {
    algorithms: Vec<String>,
    device_id: String,
    keys: std::collections::HashMap<String, String>,
    signatures: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    unsigned: Option<UnsignedDeviceInfo>,
    user_id: String,
}

#[derive(Debug, Serialize)]
pub struct UnsignedDeviceInfo {
    device_display_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    device_keys: std::collections::HashMap<String, std::collections::HashMap<String, DeviceKeys>>,
    master_keys: Option<std::collections::HashMap<String, serde_json::Value>>,
    self_signing_keys: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// POST /_matrix/federation/v1/user/keys/query
///
/// Returns the current devices and identity keys for the given users.
pub async fn post(
    State(state): State<AppState>,
    Json(payload): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, StatusCode> {
    debug!("Federation keys query request: {:?}", payload);

    let mut response_device_keys = std::collections::HashMap::new();
    let mut master_keys = std::collections::HashMap::new();
    let mut self_signing_keys = std::collections::HashMap::new();

    for (user_id, device_list) in payload.device_keys {
        debug!("Querying keys for user: {}", user_id);

        let mut user_device_keys = std::collections::HashMap::new();

        // Query devices for this user
        let query = if device_list.is_empty() {
            // Get all devices for user
            "SELECT device_id, display_name, device_keys FROM device WHERE user_id = $user_id"
        } else {
            // Get specific devices
            "SELECT device_id, display_name, device_keys FROM device WHERE user_id = $user_id AND device_id IN $device_list"
        };

        let mut result = state
            .db
            .query(query)
            .bind(("user_id", user_id.clone()))
            .bind(("device_list", device_list.clone()))
            .await
            .map_err(|e| {
                error!("Database query failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        if let Ok(devices) = result.take::<Vec<serde_json::Value>>(0) {
            for device_data in devices {
                if let (Some(device_id), device_keys) = (
                    device_data.get("device_id").and_then(|v| v.as_str()),
                    device_data.get("device_keys"),
                ) {
                    if let Some(keys_obj) = device_keys.and_then(|v| v.as_object()) {
                        // Extract device key information
                        let algorithms = keys_obj
                            .get("algorithms")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default();

                        let keys = keys_obj
                            .get("keys")
                            .and_then(|v| v.as_object())
                            .map(|obj| {
                                obj.iter()
                                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                    .collect()
                            })
                            .unwrap_or_default();

                        let signatures = keys_obj
                            .get("signatures")
                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                            .unwrap_or_default();

                        let device_keys = DeviceKeys {
                            algorithms,
                            device_id: device_id.to_string(),
                            keys,
                            signatures,
                            unsigned: device_data.get("display_name").and_then(|v| v.as_str()).map(
                                |name| {
                                    UnsignedDeviceInfo {
                                        device_display_name: Some(name.to_string()),
                                    }
                                },
                            ),
                            user_id: user_id.clone(),
                        };

                        user_device_keys.insert(device_id.to_string(), device_keys);
                    }
                }
            }
        } // Query cross-signing keys for this user
        match query_cross_signing_keys(&state, &user_id).await {
            Ok((master_key, self_signing_key)) => {
                if let Some(master) = master_key {
                    master_keys.insert(user_id.clone(), master);
                }
                if let Some(self_signing) = self_signing_key {
                    self_signing_keys.insert(user_id.clone(), self_signing);
                }
            },
            Err(e) => {
                error!("Failed to query cross-signing keys for user {}: {}", user_id, e);
                // Continue without cross-signing keys rather than failing entire request
            },
        }

        if !user_device_keys.is_empty() {
            response_device_keys.insert(user_id, user_device_keys);
        }
    }

    debug!("Federation keys query response prepared for {} users", response_device_keys.len());

    Ok(Json(QueryResponse {
        device_keys: response_device_keys,
        master_keys: if master_keys.is_empty() {
            None
        } else {
            Some(master_keys)
        },
        self_signing_keys: if self_signing_keys.is_empty() {
            None
        } else {
            Some(self_signing_keys)
        },
    }))
}

/// Query cross-signing keys for a user from the database
async fn query_cross_signing_keys(
    state: &AppState,
    user_id: &str,
) -> Result<
    (Option<serde_json::Value>, Option<serde_json::Value>),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let query = "
        SELECT key_type, keys, signatures, usage, user_id
        FROM cross_signing_keys
        WHERE user_id = $user_id
    ";

    let mut result = state.db.query(query).bind(("user_id", user_id.to_string())).await?;

    let cross_signing_records: Vec<serde_json::Value> = result.take(0)?;

    let mut master_key = None;
    let mut self_signing_key = None;

    for record in cross_signing_records {
        let key_type = record.get("key_type").and_then(|v| v.as_str()).unwrap_or("");

        match key_type {
            "master" => {
                master_key = Some(json!({
                    "keys": record.get("keys").cloned().unwrap_or_default(),
                    "signatures": record.get("signatures").cloned().unwrap_or_default(),
                    "usage": record.get("usage").cloned().unwrap_or_default(),
                    "user_id": user_id
                }));
            },
            "self_signing" => {
                self_signing_key = Some(json!({
                    "keys": record.get("keys").cloned().unwrap_or_default(),
                    "signatures": record.get("signatures").cloned().unwrap_or_default(),
                    "usage": record.get("usage").cloned().unwrap_or_default(),
                    "user_id": user_id
                }));
            },
            _ => {
                debug!("Unknown cross-signing key type: {}", key_type);
            },
        }
    }

    Ok((master_key, self_signing_key))
}
