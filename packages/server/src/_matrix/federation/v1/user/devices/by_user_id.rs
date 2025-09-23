use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use matryx_entity::{DeviceInfo, DeviceKeys};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, warn};

use crate::{AppState, auth::MatrixSessionService};

/// Cross-signing key structure
#[derive(Debug, Serialize, Deserialize)]
pub struct CrossSigningKey {
    pub keys: std::collections::HashMap<String, String>,
    pub signatures:
        Option<std::collections::HashMap<String, std::collections::HashMap<String, String>>>,
    pub usage: Vec<String>,
    pub user_id: String,
}

/// Response structure for device list endpoint
#[derive(Debug, Serialize)]
pub struct DeviceListResponse {
    pub devices: Vec<DeviceInfo>,
    pub master_key: Option<CrossSigningKey>,
    pub self_signing_key: Option<CrossSigningKey>,
    pub stream_id: i64,
    pub user_id: String,
}

/// GET /_matrix/federation/v1/user/devices/{userId}
///
/// Returns the current devices and identity keys for the given user.
/// This is used for initial device list population and resynchronization.
pub async fn get(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<DeviceListResponse>, StatusCode> {
    // Validate X-Matrix authentication header
    let _origin_server = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
    })?;

    debug!("Federation device list request for user: {}", user_id);

    // Verify the user exists locally
    let user_query = "SELECT user_id FROM users WHERE user_id = $user_id";
    let mut user_result = state
        .db
        .query(user_query)
        .bind(("user_id", user_id.clone()))
        .await
        .map_err(|e| {
            error!("Database query failed for user verification: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let users: Vec<Value> = user_result.take(0).map_err(|e| {
        error!("Failed to parse user query result: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if users.is_empty() {
        warn!("Device list requested for non-existent user: {}", user_id);
        return Err(StatusCode::NOT_FOUND);
    }

    // Query all devices for the user
    let devices_query = "
        SELECT device_id, display_name, device_keys, created_at, last_seen_ip, last_seen_ts
        FROM device
        WHERE user_id = $user_id
        ORDER BY created_at ASC
    ";

    let mut devices_result = state
        .db
        .query(devices_query)
        .bind(("user_id", user_id.clone()))
        .await
        .map_err(|e| {
            error!("Database query failed for devices: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let device_records: Vec<Value> = devices_result.take(0).map_err(|e| {
        error!("Failed to parse devices query result: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut devices = Vec::new();

    for device_record in device_records {
        let device_id = device_record
            .get("device_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let display_name = device_record
            .get("display_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse device keys
        if let Some(device_keys_value) = device_record.get("device_keys") {
            if let Ok(parsed_keys) = serde_json::from_value::<DeviceKeys>(device_keys_value.clone())
            {
                let device_info = DeviceInfo {
                    device_display_name: display_name,
                    device_id,
                    keys: parsed_keys,
                };
                devices.push(device_info);
            } else {
                warn!("Failed to parse device keys for device: {}", device_id);
            }
        }
    }

    // Query cross-signing keys
    let (master_key, self_signing_key) = match query_cross_signing_keys(&state, &user_id).await {
        Ok((master, self_signing)) => (master, self_signing),
        Err(e) => {
            error!("Failed to query cross-signing keys for user {}: {}", user_id, e);
            (None, None)
        },
    };

    // Get current stream_id for this user's device updates
    let stream_id = get_device_stream_id(&state, &user_id).await.unwrap_or(0);

    debug!("Returning device list for user {} with {} devices", user_id, devices.len());

    Ok(Json(DeviceListResponse {
        devices,
        master_key,
        self_signing_key,
        stream_id,
        user_id,
    }))
}

/// Parse X-Matrix authentication header for federation
fn parse_x_matrix_auth(headers: &HeaderMap) -> Result<String, StatusCode> {
    let auth_header = headers
        .get("authorization")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if !auth_header.starts_with("X-Matrix ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let auth_params = &auth_header[9..]; // Skip "X-Matrix "

    // Parse origin parameter
    for param in auth_params.split(',') {
        let param = param.trim();
        if let Some((key, value)) = param.split_once('=') {
            if key.trim() == "origin" {
                return Ok(value.trim().to_string());
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

/// Query cross-signing keys for a user from the database
async fn query_cross_signing_keys(
    state: &AppState,
    user_id: &str,
) -> Result<
    (Option<CrossSigningKey>, Option<CrossSigningKey>),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let query = "
        SELECT key_type, keys, signatures, usage, user_id, updated_at
        FROM cross_signing_keys
        WHERE user_id = $user_id
        ORDER BY updated_at DESC
    ";

    let mut result = state.db.query(query).bind(("user_id", user_id.to_string())).await?;

    let cross_signing_records: Vec<Value> = result.take(0)?;

    let mut master_key = None;
    let mut self_signing_key = None;

    for record in cross_signing_records {
        let key_type = record.get("key_type").and_then(|v| v.as_str()).unwrap_or("");

        match key_type {
            "master" => {
                if master_key.is_none() {
                    // Only take the most recent
                    let keys = record
                        .get("keys")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default();

                    let signatures = record
                        .get("signatures")
                        .and_then(|v| serde_json::from_value(v.clone()).ok());

                    let usage = record
                        .get("usage")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_else(|| vec!["master".to_string()]);

                    master_key = Some(CrossSigningKey {
                        keys,
                        signatures,
                        usage,
                        user_id: user_id.to_string(),
                    });
                }
            },
            "self_signing" => {
                if self_signing_key.is_none() {
                    // Only take the most recent
                    let keys = record
                        .get("keys")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_default();

                    let signatures = record
                        .get("signatures")
                        .and_then(|v| serde_json::from_value(v.clone()).ok());

                    let usage = record
                        .get("usage")
                        .and_then(|v| serde_json::from_value(v.clone()).ok())
                        .unwrap_or_else(|| vec!["self_signing".to_string()]);

                    self_signing_key = Some(CrossSigningKey {
                        keys,
                        signatures,
                        usage,
                        user_id: user_id.to_string(),
                    });
                }
            },
            "user_signing" => {
                // User-signing keys are not returned in federation API
                debug!("Ignoring user-signing key for federation response");
            },
            _ => {
                debug!("Unknown cross-signing key type: {}", key_type);
            },
        }
    }

    Ok((master_key, self_signing_key))
}

/// Get the current device stream ID for a user
async fn get_device_stream_id(
    state: &AppState,
    user_id: &str,
) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    let query = "
        SELECT stream_id
        FROM device_list_updates
        WHERE user_id = $user_id
        ORDER BY stream_id DESC
        LIMIT 1
    ";

    let mut result = state.db.query(query).bind(("user_id", user_id.to_string())).await?;

    let stream_records: Vec<Value> = result.take(0)?;

    if let Some(record) = stream_records.first() {
        if let Some(stream_id) = record.get("stream_id").and_then(|v| v.as_i64()) {
            return Ok(stream_id);
        }
    }

    // If no stream ID found, return 0 as initial value
    Ok(0)
}
