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
    federation::FederationRetryManager,
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

#[derive(Serialize, Deserialize)]
pub struct KeysQueryResponse {
    pub device_keys: std::collections::HashMap<String, std::collections::HashMap<String, Value>>,
    pub failures: std::collections::HashMap<String, Value>,
    pub master_keys: std::collections::HashMap<String, Value>,
    pub self_signing_keys: std::collections::HashMap<String, Value>,
    pub user_signing_keys: std::collections::HashMap<String, Value>,
}

#[derive(Serialize)]
pub struct FederationKeysQueryRequest {
    pub device_keys: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Debug, thiserror::Error)]
pub enum FederationError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Timeout")]
    Timeout,
    #[error("Invalid response")]
    InvalidResponse,
}

pub struct UserKeys {
    pub device_keys: std::collections::HashMap<String, Value>,
    pub master_key: Option<Value>,
    pub self_signing_key: Option<Value>,
    pub user_signing_key: Option<Value>,
}

/// POST /_matrix/client/v3/keys/query with federation support
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<KeysQueryRequest>,
) -> Result<Json<KeysQueryResponse>, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let requesting_user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        _ => return Err(StatusCode::FORBIDDEN),
    };

    let mut response = KeysQueryResponse {
        device_keys: std::collections::HashMap::new(),
        failures: std::collections::HashMap::new(),
        master_keys: std::collections::HashMap::new(),
        self_signing_keys: std::collections::HashMap::new(),
        user_signing_keys: std::collections::HashMap::new(),
    };

    // Separate local and federated users
    let mut local_users = Vec::new();
    let mut federated_users = std::collections::HashMap::new();

    for (user_id, device_ids) in &request.device_keys {
        // NEW: Check room membership for authorization
        if !can_access_user_keys(&requesting_user_id, user_id, &state).await {
            response
                .failures
                .insert(user_id.clone(), json!({"error": "Not in shared room with user"}));
            continue;
        }

        if is_local_user(user_id, &state.homeserver_name) {
            local_users.push((user_id.clone(), device_ids.clone()));
        } else {
            let server_name = extract_server_name(user_id)?;
            federated_users
                .entry(server_name)
                .or_insert_with(Vec::new)
                .push((user_id.clone(), device_ids.clone()));
        }
    }

    // Process local users
    for (user_id, device_ids) in local_users {
        match query_local_user_keys(&state.db, &user_id, &device_ids).await {
            Ok(user_keys) => {
                if !user_keys.device_keys.is_empty() {
                    response.device_keys.insert(user_id.clone(), user_keys.device_keys);
                }
                if let Some(master_key) = user_keys.master_key {
                    response.master_keys.insert(user_id.clone(), master_key);
                }
                if let Some(self_signing_key) = user_keys.self_signing_key {
                    response.self_signing_keys.insert(user_id.clone(), self_signing_key);
                }
                if let Some(user_signing_key) = user_keys.user_signing_key {
                    response.user_signing_keys.insert(user_id.clone(), user_signing_key);
                }
            },
            Err(e) => {
                response
                    .failures
                    .insert(user_id, json!({"error": format!("Failed to query keys: {}", e)}));
            },
        }
    }

    // NEW: Process federated users with actual federation implementation
    for (server_name, user_requests) in federated_users {
        match query_federated_keys(&state, &server_name, &user_requests).await {
            Ok(federated_response) => {
                response.device_keys.extend(federated_response.device_keys);
                response.master_keys.extend(federated_response.master_keys);
                response.self_signing_keys.extend(federated_response.self_signing_keys);
                response.user_signing_keys.extend(federated_response.user_signing_keys);
                response.failures.extend(federated_response.failures);
            },
            Err(e) => {
                error!("Federation query failed for server {}: {:?}", server_name, e);
                for (user_id, _) in &user_requests {
                    response.failures.insert(
                        user_id.clone(),
                        serde_json::json!({
                            "errcode": "M_FEDERATION_ERROR",
                            "error": format!("Federation query failed: {}", e)
                        }),
                    );
                }
            },
        }
    }

    info!("Keys query completed for {} users with federation support", request.device_keys.len());
    Ok(Json(response))
}

/// Query keys from federated server using Matrix federation protocol
async fn query_federated_keys(
    state: &AppState,
    server_name: &str,
    user_requests: &[(String, Vec<String>)],
) -> Result<KeysQueryResponse, FederationError> {
    // Build federation request payload
    let mut device_keys = std::collections::HashMap::new();
    for (user_id, device_ids) in user_requests {
        device_keys.insert(user_id.clone(), device_ids.clone());
    }

    let federation_request = FederationKeysQueryRequest { device_keys };

    // Make federation request to /_matrix/federation/v1/user/keys/query
    let endpoint = format!("https://{}/_matrix/federation/v1/user/keys/query", server_name);

    let request_body =
        serde_json::to_string(&federation_request).map_err(|e| FederationError::InvalidResponse)?;

    info!("Querying keys from federated server: {} for {} users", server_name, user_requests.len());

    match state
        .http_client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .header("User-Agent", format!("matryx/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .body(request_body)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<KeysQueryResponse>().await {
                    Ok(keys_response) => {
                        info!("Successfully received keys from federated server: {}", server_name);
                        Ok(keys_response)
                    },
                    Err(e) => {
                        error!("Failed to parse federation response from {}: {}", server_name, e);
                        Err(FederationError::InvalidResponse)
                    },
                }
            } else {
                error!(
                    "Federation server {} returned error status: {}",
                    server_name,
                    response.status()
                );
                Err(FederationError::Network(format!(
                    "Server returned status: {}",
                    response.status()
                )))
            }
        },
        Err(e) => {
            if e.is_timeout() {
                error!("Timeout querying federation server: {}", server_name);
                Err(FederationError::Timeout)
            } else {
                error!("Network error querying federation server {}: {}", server_name, e);
                Err(FederationError::Network(format!("Network error: {}", e)))
            }
        },
    }
}

async fn query_local_user_keys(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
    device_ids: &[String],
) -> Result<UserKeys, StatusCode> {
    let mut user_device_keys = std::collections::HashMap::new();

    if device_ids.is_empty() {
        // Query all devices for this user
        let query = "SELECT * FROM device_keys WHERE user_id = $user_id";
        let user_id_owned = user_id.to_string();
        let mut response = db
            .query(query)
            .bind(("user_id", user_id_owned))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
            let user_id_owned = user_id.to_string();
            let device_id_owned = device_id.clone();
            let mut response = db
                .query(query)
                .bind(("user_id", user_id_owned))
                .bind(("device_id", device_id_owned))
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            let device: Option<Value> =
                response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            if let Some(device_data) = device {
                if let Some(keys) = device_data.get("device_keys") {
                    user_device_keys.insert(device_id.clone(), keys.clone());
                }
            }
        }
    }

    // Query cross-signing keys for this user
    let (master_key, self_signing_key, user_signing_key) =
        query_cross_signing_keys(db, user_id).await?;

    Ok(UserKeys {
        device_keys: user_device_keys,
        master_key,
        self_signing_key,
        user_signing_key,
    })
}

async fn query_cross_signing_keys(
    db: &surrealdb::Surreal<surrealdb::engine::any::Any>,
    user_id: &str,
) -> Result<(Option<Value>, Option<Value>, Option<Value>), StatusCode> {
    let query = "SELECT * FROM cross_signing_keys WHERE user_id = $user_id";
    let user_id_owned = user_id.to_string();
    let mut response = db
        .query(query)
        .bind(("user_id", user_id_owned))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let cross_signing_keys: Vec<Value> =
        response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut master_key = None;
    let mut self_signing_key = None;
    let mut user_signing_key = None;

    for key in cross_signing_keys {
        if let Some(key_type) = key.get("key_type").and_then(|v| v.as_str()) {
            if let Some(key_data) = key.get("key_data") {
                match key_type {
                    "master" => master_key = Some(key_data.clone()),
                    "self_signing" => self_signing_key = Some(key_data.clone()),
                    "user_signing" => user_signing_key = Some(key_data.clone()),
                    _ => {},
                }
            }
        }
    }

    Ok((master_key, self_signing_key, user_signing_key))
}

async fn can_access_user_keys(
    requesting_user_id: &str,
    target_user_id: &str,
    state: &AppState,
) -> bool {
    // Users can always access their own keys
    if requesting_user_id == target_user_id {
        return true;
    }

    // Check if users share any rooms by querying room membership
    let query = r#"
        SELECT room_id FROM room_memberships
        WHERE user_id = $requesting_user_id AND membership = 'join'
        INTERSECT
        SELECT room_id FROM room_memberships
        WHERE user_id = $target_user_id AND membership = 'join'
        LIMIT 1
    "#;

    let requesting_user_owned = requesting_user_id.to_string();
    let target_user_owned = target_user_id.to_string();

    match state
        .db
        .query(query)
        .bind(("requesting_user_id", requesting_user_owned))
        .bind(("target_user_id", target_user_owned))
        .await
    {
        Ok(mut response) => {
            // If we get any results, users share at least one room
            match response.take::<Vec<Value>>(0) {
                Ok(rooms) => !rooms.is_empty(),
                Err(_) => false,
            }
        },
        Err(_) => false,
    }
}

fn is_local_user(user_id: &str, server_name: &str) -> bool {
    user_id
        .split(':')
        .nth(1)
        .map(|domain| domain == server_name)
        .unwrap_or(false)
}

fn extract_server_name(user_id: &str) -> Result<String, StatusCode> {
    user_id
        .split(':')
        .nth(1)
        .map(|s| s.to_string())
        .ok_or(StatusCode::BAD_REQUEST)
}
