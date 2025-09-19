use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};

#[derive(Deserialize)]
pub struct SignaturesUploadRequest {
    #[serde(flatten)]
    pub signatures: std::collections::HashMap<
        String,
        std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    >,
}

/// POST /_matrix/client/v3/keys/signatures/upload
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SignaturesUploadRequest>,
) -> Result<Json<Value>, StatusCode> {
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        error!("Signatures upload failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let signing_user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        _ => return Err(StatusCode::FORBIDDEN),
    };

    let mut failures = std::collections::HashMap::new();

    // Process signatures for each target user
    for (target_user_id, target_signatures) in request.signatures {
        for (key_id, signatures) in target_signatures {
            // Determine if this is a device key or cross-signing key signature
            if key_id.contains(':') {
                // Device key signature
                let parts: Vec<&str> = key_id.split(':').collect();
                if parts.len() >= 2 {
                    let device_id = parts[1].to_string();

                    // Update device key signatures
                    let query = "
                        UPDATE device_keys 
                        SET signatures = array::union(signatures, $new_signatures), updated_at = $updated_at
                        WHERE user_id = $user_id AND device_id = $device_id
                    ";

                    let result = state
                        .db
                        .query(query)
                        .bind(("user_id", target_user_id.clone()))
                        .bind(("device_id", device_id.clone()))
                        .bind(("new_signatures", json!({ signing_user_id.clone(): signatures })))
                        .bind(("updated_at", Utc::now()))
                        .await;

                    match result {
                        Ok(_) => {
                            info!(
                                "Device key signature added: signer={} target_user={} device={}",
                                signing_user_id, target_user_id, device_id
                            );
                        },
                        Err(e) => {
                            error!("Failed to update device key signatures: {}", e);
                            failures.insert(
                                key_id.clone(),
                                json!({
                                    "error": "Failed to update device key signatures"
                                }),
                            );
                        },
                    }
                }
            } else {
                // Cross-signing key signature
                let key_type = match key_id.as_str() {
                    k if k.starts_with("ed25519:") => {
                        if k.contains("master") {
                            "master"
                        } else if k.contains("self_signing") {
                            "self_signing"
                        } else if k.contains("user_signing") {
                            "user_signing"
                        } else {
                            "unknown"
                        }
                    },
                    _ => "unknown",
                };

                if key_type != "unknown" {
                    let query = "
                        UPDATE cross_signing_keys 
                        SET signatures = array::union(signatures, $new_signatures), created_at = $updated_at
                        WHERE user_id = $user_id AND key_type = $key_type
                    ";

                    let result = state
                        .db
                        .query(query)
                        .bind(("user_id", target_user_id.clone()))
                        .bind(("key_type", key_type))
                        .bind(("new_signatures", json!({ signing_user_id.clone(): signatures })))
                        .bind(("updated_at", Utc::now()))
                        .await;

                    match result {
                        Ok(_) => {
                            info!(
                                "Cross-signing key signature added: signer={} target_user={} key_type={}",
                                signing_user_id, target_user_id, key_type
                            );
                        },
                        Err(e) => {
                            error!("Failed to update cross-signing key signatures: {}", e);
                            failures.insert(
                                key_id.clone(),
                                json!({
                                    "error": "Failed to update cross-signing key signatures"
                                }),
                            );
                        },
                    }
                } else {
                    failures.insert(
                        key_id.clone(),
                        json!({
                            "error": "Unknown key type"
                        }),
                    );
                }
            }
        }
    }

    let response = if failures.is_empty() {
        json!({})
    } else {
        json!({
            "failures": failures
        })
    };

    info!("Signatures upload completed for user: {}", signing_user_id);
    Ok(Json(response))
}
