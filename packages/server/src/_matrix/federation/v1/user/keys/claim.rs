use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use matryx_surrealdb::{CryptoKeysRepository, OneTimeKeysClaim};
use tracing::{debug, error};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct ClaimRequest {
    pub one_time_keys: std::collections::HashMap<String, std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyObject {
    pub key: String,
}

#[derive(Debug, Serialize)]
pub struct ClaimResponse {
    pub one_time_keys: std::collections::HashMap<String, std::collections::HashMap<String, std::collections::HashMap<String, KeyObject>>>,
}

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

            let keys_repo = CryptoKeysRepository::new(state.db.clone());
            let mut device_keys = HashMap::new();

            // Create OneTimeKeysClaim struct for this specific device and algorithm
            let mut device_claim = HashMap::new();
            device_claim.insert(device_id.clone(), algorithm.clone());
            let mut claim_request = HashMap::new();
            claim_request.insert(user_id.clone(), device_claim);
            
            let one_time_keys_claim = OneTimeKeysClaim {
                one_time_keys: claim_request,
                timeout: Some(10000), // 10 second timeout as per Matrix spec
            };

            // Try to claim one-time key
            match keys_repo.claim_one_time_keys(&one_time_keys_claim).await {
                Ok(response) => {
                    // Extract keys from the OneTimeKeysResponse
                    if let Some(user_keys_map) = response.one_time_keys.get(&user_id)
                        && let Some(device_keys_map) = user_keys_map.get(&device_id) {
                        for (key_id, key_value) in device_keys_map {
                            if let Ok(key_obj) = serde_json::from_value::<KeyObject>(key_value.clone()) {
                                device_keys.insert(key_id.clone(), key_obj);
                            }
                        }
                    }
                    
                    // If no keys were claimed, the response will be empty - this is normal
                    if device_keys.is_empty() {
                        debug!("No one-time keys available for user {} device {} algorithm {}", user_id, device_id, algorithm);
                    }
                },
                Err(e) => {
                    error!("Failed to claim one-time keys: {}", e);
                },
            }

            if !device_keys.is_empty() {
                user_keys.insert(device_id, device_keys);
            }
        }

        if !user_keys.is_empty() {
            response_keys.insert(user_id, user_keys);
        }
    }

    debug!("Federation keys claim response prepared for {} users", response_keys.len());

    Ok(Json(ClaimResponse { one_time_keys: response_keys }))
}