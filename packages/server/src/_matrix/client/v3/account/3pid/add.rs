use axum::{
    extract::State,
    http::StatusCode,
    response::{Json, Response, IntoResponse},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::AppState;
use crate::auth::uia::{UiaAuth, UiaFlow, ThreepidCredentials};
use crate::auth::authenticated_user::AuthenticatedUser;
use crate::error::matrix_errors::MatrixError;
use matryx_surrealdb::repository::{
    third_party::ThirdPartyRepository,
    third_party_validation_session::{ThirdPartyValidationSessionRepository, ThirdPartyValidationSessionRepositoryTrait}
};

#[derive(Debug, Deserialize)]
pub struct ThreePidAddRequest {
    pub threepid_creds: ThreepidCredentials,
    pub bind: Option<bool>,
    pub auth: Option<UiaAuth>,
}

/// POST /_matrix/client/v3/account/3pid/add
pub async fn post(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
    Json(request): Json<ThreePidAddRequest>,
) -> Result<Json<Value>, Response> {
    info!("3PID add request received");

    // Validate ThreepidCredentials
    if request.threepid_creds.sid.is_empty() || request.threepid_creds.client_secret.is_empty() {
        warn!("Invalid threepid credentials: missing sid or client_secret");
        return Err(StatusCode::BAD_REQUEST);
    }

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
            }
        };
        match uia_service.process_auth(&session_id, uia_auth).await {
            Ok(_) => {
                info!("UIA authentication successful for 3PID addition");
                // Authentication passed, continue with 3PID addition
                
                let third_party_repo = ThirdPartyRepository::new(state.db.clone());
                let session_repo = ThirdPartyValidationSessionRepository::new(state.db.clone());

                // Validate ThreepidCredentials using existing session management
                let session = session_repo.get_session_by_id_and_secret(
                    &request.threepid_creds.sid,
                    &request.threepid_creds.client_secret
                ).await.map_err(|_| MatrixError::Unknown.into_response())?
                .ok_or(MatrixError::InvalidParam.into_response())?;

                // Verify session is verified
                if !session.verified {
                    return Err(MatrixError::SessionNotValidated.into_response());
                }

                // Add 3PID to user account using existing infrastructure
                third_party_repo.add_third_party_identifier(
                    &authenticated_user.user_id,
                    &session.medium,
                    &session.address,
                    true // validated
                ).await.map_err(|_| MatrixError::Unknown.into_response())?;
                
                // Return success response per Matrix spec
                return Ok(Json(json!({})));
            },
            Err(uia_error) => {
                warn!("UIA authentication failed for 3PID addition: {:?}", uia_error);
                // Return UIA error response per Matrix spec
                return Ok(Json(json!({
                    "flows": uia_error.flows,
                    "params": uia_error.params,
                    "session": uia_error.session,
                    "completed": uia_error.completed,
                    "error": uia_error.error,
                    "errcode": uia_error.errcode
                })));
            }
        }
    } else {
        // No auth data provided - start UIA flow per Matrix spec
        info!("No UIA auth provided for 3PID addition, starting UIA flow");

        // Use centralized UIA service from AppState
        let uia_service = &state.uia_service;

        // Define required authentication flows for 3PID addition
        let flows = vec![
            UiaFlow {
                stages: vec!["m.login.password".to_string()]
            },
            UiaFlow {
                stages: vec![
                    "m.login.recaptcha".to_string(),
                    "m.login.password".to_string()
                ]
            },
        ];

        // Start UIA session
        let session = uia_service.start_session(
            Some(&authenticated_user.user_id), // user_id from authenticated user
            None, // device_id not required for 3PID operations
            flows.clone(),
            std::collections::HashMap::new(),
        ).await.map_err(|e| {
            error!("Failed to start UIA session: {:?}", e);
            MatrixError::Unknown.into_response()
        })?;

        // Return UIA challenge per Matrix spec
        return Ok(Json(json!({
            "flows": flows,
            "params": {},
            "session": session.session_id,
            "completed": session.completed,
        })));
    }
}