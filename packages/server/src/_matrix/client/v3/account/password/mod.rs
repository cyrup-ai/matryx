use axum::{
    extract::State,
    response::{IntoResponse, Json, Response},
};
use bcrypt::hash;
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::AppState;
use crate::auth::authenticated_user::AuthenticatedUser;
use crate::auth::uia::{UiaAuth, UiaFlow};
use crate::error::matrix_errors::MatrixError;
use matryx_surrealdb::repository::{SessionRepository, UserRepository};

#[derive(Debug, Deserialize)]
pub struct PasswordChangeRequest {
    pub new_password: String,
    pub logout_devices: Option<bool>,
    pub auth: Option<UiaAuth>,
}

/// POST /_matrix/client/v3/account/password
pub async fn post(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
    Json(request): Json<PasswordChangeRequest>,
) -> Result<Json<Value>, Response> {
    info!("Password change request received");

    // Check if UIA auth data is provided
    if let Some(uia_auth) = request.auth {
        // Use centralized UIA service from AppState
        let uia_service = &state.uia_service;

        // Validate the authentication
        let session_id = match &uia_auth.session {
            Some(id) if !id.is_empty() => id.clone(),
            _ => {
                warn!("Invalid or missing session ID in UIA auth");
                return Err(MatrixError::InvalidParam.into_response());
            },
        };
        match uia_service.process_auth(&session_id, uia_auth).await {
            Ok(_) => {
                info!("UIA authentication successful for password change");
                // Authentication passed, continue with password change

                // Hash new password using bcrypt
                let password_hash = hash(&request.new_password, 12)
                    .map_err(|_| MatrixError::Unknown.into_response())?;

                // Update user password in database
                let user_repo = UserRepository::new(state.db.clone());
                let mut user = user_repo
                    .get_by_id(&authenticated_user.user_id)
                    .await
                    .map_err(|_| MatrixError::Unknown.into_response())?
                    .ok_or(MatrixError::NotFound.into_response())?;

                user.password_hash = password_hash;
                user_repo
                    .update(&user)
                    .await
                    .map_err(|_| MatrixError::Unknown.into_response())?;

                // If logout_devices is true, logout all other devices
                if request.logout_devices.unwrap_or(false) {
                    let session_repo = SessionRepository::new(state.db.clone());
                    session_repo
                        .deactivate_all_user_sessions(&authenticated_user.user_id)
                        .await
                        .map_err(|_| MatrixError::Unknown.into_response())?;
                }

                // Return success response per Matrix spec
                Ok(Json(json!({})))
            },
            Err(uia_error) => {
                warn!("UIA authentication failed for password change: {:?}", uia_error);
                // Return UIA error response per Matrix spec
                Ok(Json(json!({
                    "flows": uia_error.flows,
                    "params": uia_error.params,
                    "session": uia_error.session,
                    "completed": uia_error.completed,
                    "error": uia_error.error,
                    "errcode": uia_error.errcode
                })))
            },
        }
    } else {
        // No auth data provided - start UIA flow per Matrix spec
        info!("No UIA auth provided for password change, starting UIA flow");

        // Use centralized UIA service from AppState
        let uia_service = &state.uia_service;

        // Define required authentication flows for password change
        let flows = vec![
            UiaFlow { stages: vec!["m.login.password".to_string()] },
            UiaFlow {
                stages: vec![
                    "m.login.recaptcha".to_string(),
                    "m.login.password".to_string(),
                ],
            },
        ];

        // Start UIA session
        let session = uia_service
            .start_session(
                Some(&authenticated_user.user_id), // user_id from authenticated user
                None,                              // device_id not required for password change
                flows.clone(),
                std::collections::HashMap::new(),
            )
            .await
            .map_err(|e| {
                error!("Failed to start UIA session: {:?}", e);
                MatrixError::Unknown.into_response()
            })?;

        // Return UIA challenge per Matrix spec
        Ok(Json(json!({
            "flows": flows,
            "params": {},
            "session": session.session_id,
            "completed": session.completed,
        })))
    }
}

pub mod email;
pub mod msisdn;
