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
use matryx_surrealdb::repository::KeysRepository;

#[derive(Deserialize)]
pub struct KeysQueryRequest {
    pub device_keys: std::collections::HashMap<String, Vec<String>>,
    pub timeout: Option<u64>,
}

// Use DeviceKeys from entity package (Matrix specification types)
use matryx_entity::types::DeviceKeys;

#[derive(Serialize, Deserialize)]
pub struct KeysQueryResponse {
    pub device_keys:
        std::collections::HashMap<String, std::collections::HashMap<String, DeviceKeys>>,
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
    pub device_keys: std::collections::HashMap<String, DeviceKeys>,
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

    // Handle timeout parameter (Project Matrix spec: wait time for federation queries)
    let federation_timeout_ms = request.timeout.unwrap_or(10000); // Default 10 seconds per spec
    if federation_timeout_ms > 0 {
        info!("Keys query federation timeout: {}ms", federation_timeout_ms);
    }

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
    let keys_repo = KeysRepository::new(state.db.clone());
    for (user_id, device_ids) in local_users {
        match query_local_user_keys(&keys_repo, &user_id, &device_ids).await {
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

    let request_body = serde_json::to_string(&federation_request)
        .map_err(|_e| FederationError::InvalidResponse)?;

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
    keys_repo: &KeysRepository,
    user_id: &str,
    device_ids: &[String],
) -> Result<UserKeys, StatusCode> {
    if device_ids.is_empty() {
        // Query all devices for this user
        keys_repo
            .query_local_user_keys_all_devices(user_id)
            .await
            .map(|db_keys| UserKeys {
                device_keys: db_keys.device_keys,
                master_key: db_keys.master_key,
                self_signing_key: db_keys.self_signing_key,
                user_signing_key: db_keys.user_signing_key,
            })
            .map_err(|e| {
                error!("Failed to query all user keys: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })
    } else {
        // Query specific devices
        keys_repo
            .query_local_user_keys_specific_devices(user_id, device_ids)
            .await
            .map(|db_keys| UserKeys {
                device_keys: db_keys.device_keys,
                master_key: db_keys.master_key,
                self_signing_key: db_keys.self_signing_key,
                user_signing_key: db_keys.user_signing_key,
            })
            .map_err(|e| {
                error!("Failed to query specific user keys: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })
    }
}

// This function is no longer needed as it's handled by the repository

async fn can_access_user_keys(
    requesting_user_id: &str,
    target_user_id: &str,
    state: &AppState,
) -> bool {
    let keys_repo = KeysRepository::new(state.db.clone());

    match keys_repo.can_access_user_keys(requesting_user_id, target_user_id).await {
        Ok(can_access) => can_access,
        Err(e) => {
            error!("Failed to check user key access: {}", e);
            false
        },
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
