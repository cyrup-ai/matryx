use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use bcrypt::verify;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use tracing::{error, info};

use crate::AppState;
use crate::auth::uia::{UiaAuthRequest, UiaAuthResponse};
use matryx_surrealdb::repository::{ProfileManagementService, UserRepository, uia::UiaFlow};

#[derive(Deserialize)]
pub struct DeactivateAccountRequest {
    #[serde(flatten)]
    pub uia_request: UiaAuthRequest,
    #[serde(default)]
    pub erase: bool, // Whether to erase all user data
}

#[derive(Serialize)]
pub struct DeactivateAccountResponse {
    pub id_server_unbind_result: String,
}

pub async fn deactivate_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeactivateAccountRequest>,
) -> Result<Json<DeactivateAccountResponse>, StatusCode> {
    // Extract and validate access token
    let access_token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token and get user context
    let token_info = state
        .session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user_id = &token_info.user_id;

    // Handle User Interactive Authentication (UIA) flow
    match handle_deactivation_uia(&state, user_id, &request.uia_request).await {
        Ok(()) => {
            // UIA completed successfully, proceed with deactivation
            info!("UIA completed for account deactivation: {}", user_id);
        },
        Err(uia_response) => {
            // UIA required or failed - return UIA challenge
            return Ok(Json(DeactivateAccountResponse {
                id_server_unbind_result: serde_json::to_string(&uia_response)
                    .unwrap_or_else(|_| "uia_required".to_string()),
            }));
        },
    }

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Deactivate account using ProfileManagementService
    match profile_service.deactivate_account(user_id, request.erase).await {
        Ok(()) => {
            info!("Account successfully deactivated: {} (erase: {})", user_id, request.erase);
        },
        Err(e) => {
            error!("Failed to deactivate account {}: {}", user_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    }

    Ok(Json(DeactivateAccountResponse { id_server_unbind_result: "success".to_string() }))
}

/// Handle UIA flow for account deactivation according to Matrix specification
async fn handle_deactivation_uia(
    state: &AppState,
    user_id: &str,
    uia_request: &UiaAuthRequest,
) -> Result<(), UiaAuthResponse> {
    // Define required authentication flows for account deactivation
    let flows = vec![UiaFlow { stages: vec!["m.login.password".to_string()] }];

    let mut params = HashMap::new();
    params.insert("user_id".to_string(), serde_json::Value::String(user_id.to_string()));

    // Check if this is the initial request (no auth provided)
    if uia_request.auth.is_none() && uia_request.session.is_none() {
        // Start new UIA session
        match state
            .uia_service
            .start_session(
                Some(user_id),
                None, // device_id not required for account deactivation
                flows.clone(),
                params.clone(),
            )
            .await
        {
            Ok(session) => {
                return Err(UiaAuthResponse {
                    flows,
                    params,
                    session: session.session_id,
                    completed: Some(Vec::new()),
                    error: None,
                    errcode: None,
                });
            },
            Err(e) => {
                error!("Failed to start UIA session for {}: {}", user_id, e);
                return Err(UiaAuthResponse {
                    flows,
                    params,
                    session: format!("error_{}", uuid::Uuid::new_v4()),
                    completed: None,
                    error: Some("Failed to start authentication".to_string()),
                    errcode: Some("M_UNKNOWN".to_string()),
                });
            },
        }
    }

    // Handle authentication attempt
    if let Some(auth) = &uia_request.auth {
        let session_id = uia_request.session.as_ref().ok_or_else(|| UiaAuthResponse {
            flows: flows.clone(),
            params: params.clone(),
            session: "".to_string(),
            completed: None,
            error: Some("Session required".to_string()),
            errcode: Some("M_MISSING_PARAM".to_string()),
        })?;

        // Validate authentication based on type
        match auth.auth_type.as_str() {
            "m.login.password" => {
                // Extract password from auth data
                let password =
                    auth.auth_data.get("password").and_then(|p| p.as_str()).ok_or_else(|| {
                        UiaAuthResponse {
                            flows: flows.clone(),
                            params: params.clone(),
                            session: session_id.clone(),
                            completed: None,
                            error: Some("Password required".to_string()),
                            errcode: Some("M_MISSING_PARAM".to_string()),
                        }
                    })?;

                // Get user from database to verify password
                let user_repo = UserRepository::new(state.db.clone());
                let user = match user_repo.get_by_id(user_id).await {
                    Ok(Some(user)) => user,
                    Ok(None) => {
                        error!("User not found during UIA: {}", user_id);
                        return Err(UiaAuthResponse {
                            flows,
                            params,
                            session: session_id.clone(),
                            completed: None,
                            error: Some("User not found".to_string()),
                            errcode: Some("M_FORBIDDEN".to_string()),
                        });
                    },
                    Err(e) => {
                        error!("Database error during UIA for {}: {}", user_id, e);
                        return Err(UiaAuthResponse {
                            flows,
                            params,
                            session: session_id.clone(),
                            completed: None,
                            error: Some("Authentication failed".to_string()),
                            errcode: Some("M_UNKNOWN".to_string()),
                        });
                    },
                };

                // Validate password using bcrypt
                let password_valid =
                    verify(password, &user.password_hash).map_err(|bcrypt_error| {
                        error!(
                            "Bcrypt verification error during UIA for {}: {:?}",
                            user_id, bcrypt_error
                        );
                        UiaAuthResponse {
                            flows: flows.clone(),
                            params: params.clone(),
                            session: session_id.clone(),
                            completed: None,
                            error: Some("Authentication failed".to_string()),
                            errcode: Some("M_UNKNOWN".to_string()),
                        }
                    })?;

                if password_valid {
                    // Password validated - complete UIA flow
                    info!("UIA password validation successful for {}", user_id);
                    return Ok(());
                } else {
                    return Err(UiaAuthResponse {
                        flows,
                        params,
                        session: session_id.clone(),
                        completed: Some(Vec::new()),
                        error: Some("Invalid password".to_string()),
                        errcode: Some("M_FORBIDDEN".to_string()),
                    });
                }
            },
            _ => {
                return Err(UiaAuthResponse {
                    flows,
                    params,
                    session: uia_request.session.clone().unwrap_or_default(),
                    completed: None,
                    error: Some("Unsupported authentication type".to_string()),
                    errcode: Some("M_UNKNOWN".to_string()),
                });
            },
        }
    }

    // Should not reach here in normal flow
    Err(UiaAuthResponse {
        flows,
        params,
        session: "".to_string(),
        completed: None,
        error: Some("Invalid authentication state".to_string()),
        errcode: Some("M_UNKNOWN".to_string()),
    })
}

// HTTP method handler for main.rs routing
pub use deactivate_account as post;
