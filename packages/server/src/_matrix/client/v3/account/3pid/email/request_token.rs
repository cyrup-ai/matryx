use axum::{
    extract::State,
    http::StatusCode,
    response::{Json, Response, IntoResponse},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};
use uuid::Uuid;
use chrono::{Utc, Duration};

use crate::AppState;
use crate::error::matrix_errors::MatrixError;
use matryx_surrealdb::repository::{
    third_party::ThirdPartyRepository,
    third_party_validation_session::{ThirdPartyValidationSessionRepository, ThirdPartyValidationSessionRepositoryTrait}
};
use matryx_entity::types::third_party_validation_session::ThirdPartyValidationSession;

#[derive(Debug, Deserialize)]
pub struct EmailRequestTokenRequest {
    pub client_secret: String,
    pub email: String,
    pub send_attempt: u32,
    pub next_link: Option<String>,
    pub id_server: Option<String>,
    pub id_access_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmailRequestTokenResponse {
    pub sid: String,
    pub submit_url: Option<String>,
}

/// POST /_matrix/client/v3/account/3pid/email/requestToken
pub async fn post(
    State(state): State<AppState>,
    Json(request): Json<EmailRequestTokenRequest>,
) -> Result<Json<EmailRequestTokenResponse>, Response> {
    info!("Email token request for email: {}", request.email);

    // Validate email format
    if !request.email.contains('@') {
        return Err(MatrixError::InvalidParam.into_response());
    }
    
    // Check if email already associated per Matrix spec
    let third_party_repo = ThirdPartyRepository::new(state.db.clone());
    if third_party_repo.find_user_by_third_party("email", &request.email).await
        .map_err(|_| MatrixError::Unknown.into_response())?.is_some() {
        return Err(MatrixError::ThreepidInUse.into_response());
    }
    
    // Create validation session using existing infrastructure
    let session_repo = ThirdPartyValidationSessionRepository::new(state.db.clone());
    let session_id = Uuid::new_v4().to_string();
    let verification_token = Uuid::new_v4().to_string();
    let expires_at = (Utc::now() + Duration::hours(24)).timestamp();
    
    let session = ThirdPartyValidationSession::new(
        session_id,
        request.client_secret.clone(),
        "email".to_string(),
        request.email.clone(),
        verification_token,
        expires_at,
    );
    
    session_repo.create_session(&session).await
        .map_err(|_| MatrixError::Unknown.into_response())?;
    
    // Send verification email if email service is available
    if let Some(email_service) = &state.email_service {
        if let Err(e) = email_service.send_verification_email(
            &request.email,
            &session.verification_token,
            &session.session_id,
        ).await {
            error!("Failed to send verification email to {}: {}", request.email, e);
            // Continue anyway - session is created, user can retry
        } else {
            info!("Verification email sent to {}", request.email);
        }
    } else {
        warn!("Email service not available - verification email not sent");
    }
    
    let response = EmailRequestTokenResponse {
        sid: session.session_id,
        submit_url: Some(format!("{}/_matrix/client/v3/account/3pid/email/submit_token", 
                                state.homeserver_name)),
    };

    info!("Email verification session created with ID: {}", response.sid);
    Ok(Json(response))
}