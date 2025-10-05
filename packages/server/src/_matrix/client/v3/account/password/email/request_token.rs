use axum::{
    extract::State,
    response::{IntoResponse, Json, Response},
};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use uuid::Uuid;

use crate::AppState;
use crate::error::matrix_errors::MatrixError;
use matryx_entity::types::third_party_validation_session::ThirdPartyValidationSession;
use matryx_surrealdb::repository::{
    third_party::ThirdPartyRepository,
    third_party_validation_session::{
        ThirdPartyValidationSessionRepository, ThirdPartyValidationSessionRepositoryTrait,
    },
};

#[derive(Debug, Deserialize)]
pub struct PasswordEmailRequestTokenRequest {
    pub client_secret: String,
    pub email: String,
    pub send_attempt: u32,
    pub next_link: Option<String>,
    pub id_server: Option<String>,
    pub id_access_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PasswordEmailRequestTokenResponse {
    pub sid: String,
    pub submit_url: Option<String>,
}

/// POST /_matrix/client/v3/account/password/email/requestToken
pub async fn post(
    State(state): State<AppState>,
    Json(request): Json<PasswordEmailRequestTokenRequest>,
) -> Result<Json<PasswordEmailRequestTokenResponse>, Response> {
    info!(
        "Password reset email token request for email: {} (attempt: {})",
        request.email, request.send_attempt
    );

    // Validate email format
    if !request.email.contains('@') {
        return Err(MatrixError::InvalidParam.into_response());
    }

    // Check if email is associated with an account (required for password reset)
    let third_party_repo = ThirdPartyRepository::new(state.db.clone());
    if third_party_repo
        .find_user_by_third_party("email", &request.email)
        .await
        .map_err(|_| MatrixError::Unknown.into_response())?
        .is_none()
    {
        return Err(MatrixError::NotFound.into_response());
    }

    // Create validation session using existing infrastructure
    let session_repo = ThirdPartyValidationSessionRepository::new(state.db.clone());
    let session_id = Uuid::new_v4().to_string();
    let verification_token = Uuid::new_v4().to_string();
    let expires_at = (Utc::now() + Duration::hours(24)).timestamp();

    // Log optional fields for audit trail (id_server and id_access_token are deprecated in Matrix spec)
    if let Some(ref next_link) = request.next_link {
        info!("Next link requested: {}", next_link);
    }
    if request.id_server.is_some() || request.id_access_token.is_some() {
        info!("Identity server parameters provided (deprecated in Matrix spec)");
    }

    let session = ThirdPartyValidationSession::new(
        session_id,
        request.client_secret.clone(),
        "email".to_string(),
        request.email.clone(),
        verification_token,
        expires_at,
    );

    session_repo
        .create_session(&session)
        .await
        .map_err(|_| MatrixError::Unknown.into_response())?;

    // Send password reset email if email service is available
    if let Some(email_service) = &state.email_service {
        if let Err(e) = email_service.send_password_reset_email(
            &request.email,
            &session.verification_token,
            &session.session_id,
        ).await {
            error!("Failed to send password reset email to {}: {}", request.email, e);
            return Err(MatrixError::Unknown.into_response());
        } else {
            info!("Password reset email sent to {}", request.email);
        }
    } else {
        error!("Email service not available - cannot send password reset");
        return Err(MatrixError::Unknown.into_response());
    }

    let response = PasswordEmailRequestTokenResponse {
        sid: session.session_id,
        submit_url: Some(format!(
            "{}/_matrix/client/v3/account/password/email/submit_token",
            state.homeserver_name
        )),
    };

    info!("Password reset email verification session created with ID: {}", response.sid);
    Ok(Json(response))
}
