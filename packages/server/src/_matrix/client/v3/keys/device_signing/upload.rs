use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{error, info};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
    federation::device_management::CrossSigningKey,
};

#[derive(Deserialize)]
pub struct DeviceSigningUploadRequest {
    pub master_key: Option<CrossSigningKey>,
    pub self_signing_key: Option<CrossSigningKey>,
    pub user_signing_key: Option<CrossSigningKey>,
    pub auth: Option<Value>,
}

fn validate_cross_signing_key(
    key: &CrossSigningKey,
    user_id: &str,
    expected_usage: &str,
) -> Result<(), StatusCode> {
    // Validate user_id matches
    if key.user_id != user_id {
        error!("Cross-signing key user_id mismatch");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate usage
    if !key.usage.contains(&expected_usage.to_string()) {
        error!("Cross-signing key missing required usage: {}", expected_usage);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate key format
    if key.keys.is_empty() {
        error!("Cross-signing key missing keys");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate signature exists
    if let Some(signatures) = &key.signatures {
        if !signatures.contains_key(user_id) {
            error!("Cross-signing key missing user signature");
            return Err(StatusCode::BAD_REQUEST);
        }
    } else {
        error!("Cross-signing key missing signatures");
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(())
}

/// POST /_matrix/client/v3/keys/device_signing/upload
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeviceSigningUploadRequest>,
) -> Result<Response, StatusCode> {
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        error!("Device signing upload failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        _ => return Err(StatusCode::FORBIDDEN),
    };

    // Handle User Interactive Authentication (UIA) if provided
    // According to Matrix spec, device signing key upload may require UIA for cross-signing setup
    if let Some(auth_data) = &request.auth {
        info!(
            "Device signing upload with UIA auth data for user: {} - Type: {}",
            user_id,
            auth_data.get("type").and_then(|v| v.as_str()).unwrap_or("unknown")
        );

        // Extract auth type and session from auth data
        let auth_type = auth_data.get("type")
            .and_then(|v| v.as_str())
            .ok_or(StatusCode::BAD_REQUEST)?;

        let session_id = auth_data.get("session")
            .and_then(|v| v.as_str())
            .ok_or(StatusCode::BAD_REQUEST)?;

        // Parse auth data into UiaAuth struct
        let uia_auth = crate::auth::uia::UiaAuth {
            auth_type: auth_type.to_string(),
            session: Some(session_id.to_string()),
            auth_data: {
                let mut data = std::collections::HashMap::new();
                if let Some(obj) = auth_data.as_object() {
                    for (key, value) in obj {
                        if key != "type" && key != "session" {
                            data.insert(key.clone(), value.clone());
                        }
                    }
                }
                data
            },
        };

        // Process UIA authentication
        match state.uia_service.process_auth(session_id, uia_auth).await {
            Ok(uia_response) => {
                // Check if UIA flow is completed
                if uia_response.completed.as_ref().map(|c| !c.is_empty()).unwrap_or(false) {
                    // Get the session to check if it's truly completed
                    let session = state.uia_service.uia_repo.get_session(session_id).await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                    if let Some(session) = session {
                        if !session.completed {
                            // More stages required - return 401 with updated completed stages
                            return Ok((StatusCode::UNAUTHORIZED, Json(json!({
                                "flows": uia_response.flows,
                                "params": uia_response.params,
                                "session": uia_response.session,
                                "completed": uia_response.completed,
                                "errcode": "M_FORBIDDEN",
                                "error": "Additional authentication required"
                            }))).into_response());
                        }
                        // Session is completed - delete it and proceed with request
                        let _ = state.uia_service.uia_repo.delete_session(session_id).await;
                    }
                } else {
                    // First stage completed but more required
                    return Ok((StatusCode::UNAUTHORIZED, Json(json!({
                        "flows": uia_response.flows,
                        "params": uia_response.params,
                        "session": uia_response.session,
                        "completed": uia_response.completed,
                        "errcode": "M_FORBIDDEN",
                        "error": "Additional authentication required"
                    }))).into_response());
                }
            },
            Err(uia_error) => {
                // Authentication failed or session issue
                return Ok((StatusCode::UNAUTHORIZED, Json(json!({
                    "flows": uia_error.flows,
                    "params": uia_error.params,
                    "session": uia_error.session,
                    "completed": uia_error.completed,
                    "errcode": uia_error.errcode,
                    "error": uia_error.error
                }))).into_response());
            }
        }
    } else {
        // No auth data provided - initiate UIA flow
        use crate::auth::uia::UiaFlow;

        let flows = vec![
            UiaFlow {
                stages: vec!["m.login.password".to_string()],
            }
        ];

        let params = std::collections::HashMap::new();

        // Start new UIA session
        let session = state.uia_service.start_session(
            Some(&user_id),
            None,
            flows.clone(),
            params.clone(),
        ).await.map_err(|e| {
            error!("Failed to start UIA session: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        // Return 401 with UIA flow information
        return Ok((StatusCode::UNAUTHORIZED, Json(json!({
            "flows": flows,
            "params": params,
            "session": session.session_id,
            "errcode": "M_FORBIDDEN",
            "error": "User-interactive authentication required"
        }))).into_response());
    }

    // Store master key
    if let Some(master_key) = request.master_key {
        validate_cross_signing_key(&master_key, &user_id, "master")?;

        let _: Option<CrossSigningKey> = state
            .db
            .create(("cross_signing_keys", format!("{}:master", user_id)))
            .content(json!({
                "user_id": user_id,
                "key_type": "master",
                "key_data": master_key,
                "signatures": master_key.signatures,
                "created_at": Utc::now()
            }))
            .await
            .map_err(|e| {
                error!("Failed to store master key: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!("Master key uploaded for user: {}", user_id);
    }

    // Store self-signing key
    if let Some(self_signing_key) = request.self_signing_key {
        validate_cross_signing_key(&self_signing_key, &user_id, "self_signing")?;

        let _: Option<CrossSigningKey> = state
            .db
            .create(("cross_signing_keys", format!("{}:self_signing", user_id)))
            .content(json!({
                "user_id": user_id,
                "key_type": "self_signing",
                "key_data": self_signing_key,
                "signatures": self_signing_key.signatures,
                "created_at": Utc::now()
            }))
            .await
            .map_err(|e| {
                error!("Failed to store self-signing key: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!("Self-signing key uploaded for user: {}", user_id);
    }

    // Store user-signing key
    if let Some(user_signing_key) = request.user_signing_key {
        validate_cross_signing_key(&user_signing_key, &user_id, "user_signing")?;

        let _: Option<CrossSigningKey> = state
            .db
            .create(("cross_signing_keys", format!("{}:user_signing", user_id)))
            .content(json!({
                "user_id": user_id,
                "key_type": "user_signing",
                "key_data": user_signing_key,
                "signatures": user_signing_key.signatures,
                "created_at": Utc::now()
            }))
            .await
            .map_err(|e| {
                error!("Failed to store user-signing key: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        info!("User-signing key uploaded for user: {}", user_id);
    }

    Ok(Json(json!({})).into_response())
}
