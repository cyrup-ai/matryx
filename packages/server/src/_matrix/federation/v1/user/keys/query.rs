use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error};

use crate::AppState;
use matryx_surrealdb::repository::{CrossSigningRepository, DeviceRepository};
// Use fully qualified path to avoid import conflicts
use matryx_entity::types::DeviceKeys;

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    device_keys: std::collections::HashMap<String, Vec<String>>,
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

        // Query devices for this user using repository
        let device_repo = DeviceRepository::new(state.db.clone());

        let device_keys_result = if device_list.is_empty() {
            device_repo.get_user_device_keys_for_federation_batch(&user_id).await
        } else {
            device_repo
                .get_device_keys_for_federation(&user_id, &device_list)
                .await
                .map(|response| response.device_keys.get(&user_id).cloned().unwrap_or_default())
        };

        match device_keys_result {
            Ok(device_keys_map) => {
                for (device_id, repo_device_keys) in device_keys_map {
                    let device_keys = matryx_entity::types::DeviceKeys {
                        algorithms: repo_device_keys.algorithms,
                        device_id: repo_device_keys.device_id,
                        keys: repo_device_keys.keys,
                        signatures: repo_device_keys.signatures,
                        unsigned: repo_device_keys.unsigned,
                        user_id: repo_device_keys.user_id,
                    };

                    user_device_keys.insert(device_id, device_keys);
                }
            },
            Err(e) => {
                error!("Failed to query device keys for user {}: {}", user_id, e);
                // Continue without device keys rather than failing entire request
            },
        }

        // Query cross-signing keys for this user using repository
        let cross_signing_repo = CrossSigningRepository::new(state.db.clone());

        match get_federation_cross_signing_keys(&cross_signing_repo, &user_id).await {
            Ok((master_key, self_signing_key)) => {
                if let Some(master) = master_key {
                    master_keys.insert(
                        user_id.clone(),
                        json!({
                            "keys": master.keys,
                            "signatures": master.signatures,
                            "usage": master.usage,
                            "user_id": master.user_id
                        }),
                    );
                }
                if let Some(self_signing) = self_signing_key {
                    self_signing_keys.insert(
                        user_id.clone(),
                        json!({
                            "keys": self_signing.keys,
                            "signatures": self_signing.signatures,
                            "usage": self_signing.usage,
                            "user_id": self_signing.user_id
                        }),
                    );
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

/// Get cross-signing keys for federation from repository
async fn get_federation_cross_signing_keys(
    cross_signing_repo: &CrossSigningRepository,
    user_id: &str,
) -> Result<
    (
        Option<matryx_surrealdb::repository::cross_signing::CrossSigningKey>,
        Option<matryx_surrealdb::repository::cross_signing::CrossSigningKey>,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let master_key = cross_signing_repo
        .get_master_key(user_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let self_signing_key = cross_signing_repo
        .get_self_signing_key(user_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok((master_key, self_signing_key))
}
