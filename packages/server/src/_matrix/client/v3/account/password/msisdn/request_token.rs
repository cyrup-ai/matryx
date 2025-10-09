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
pub struct PasswordMsisdnRequestTokenRequest {
    pub client_secret: String,
    pub country: String,
    pub phone_number: String,
    pub send_attempt: u32,
    pub next_link: Option<String>,
    pub id_server: Option<String>,
    pub id_access_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PasswordMsisdnRequestTokenResponse {
    pub sid: String,
    pub submit_url: Option<String>,
}

/// POST /_matrix/client/v3/account/password/msisdn/requestToken
pub async fn post(
    State(state): State<AppState>,
    Json(request): Json<PasswordMsisdnRequestTokenRequest>,
) -> Result<Json<PasswordMsisdnRequestTokenResponse>, Response> {
    // Construct international format phone number
    let phone_number = format!("+{}{}", request.country, request.phone_number);
    
    info!(
        "Password reset SMS token request for phone: {} (attempt: {})",
        phone_number, request.send_attempt
    );
    
    // Check if SMS is enabled
    if !state.config.sms_config.enabled {
        error!("SMS verification disabled - cannot send password reset SMS");
        return Err(MatrixError::Unknown.into_response());
    }
    
    // Check if phone number is associated with an account (required for password reset)
    let third_party_repo = ThirdPartyRepository::new(state.db.clone());
    if third_party_repo
        .find_user_by_third_party("msisdn", &phone_number)
        .await
        .map_err(|e| {
            error!("Failed to query third party repository: {:?}", e);
            MatrixError::Unknown.into_response()
        })?
        .is_none()
    {
        info!("Phone number not associated with any account: {}", phone_number);
        return Err(MatrixError::NotFound.into_response());
    }
    
    // Create validation session
    let session_repo = ThirdPartyValidationSessionRepository::new(state.db.clone());
    let session_id = Uuid::new_v4().to_string();
    
    // Generate short numeric verification code for SMS (not long token)
    let verification_code = generate_verification_code();
    
    // SMS codes expire faster (10 minutes vs 24 hours for email)
    let expires_at = (Utc::now() + Duration::minutes(10)).timestamp();
    
    let session = ThirdPartyValidationSession::new(
        session_id,
        request.client_secret.clone(),
        "msisdn".to_string(),
        phone_number.clone(),
        verification_code.clone(),
        expires_at,
    );
    
    session_repo
        .create_session(&session)
        .await
        .map_err(|e| {
            error!("Failed to create validation session: {:?}", e);
            MatrixError::Unknown.into_response()
        })?;
    
    // Send SMS using existing infrastructure
    send_password_reset_sms(&phone_number, &verification_code, &state).await?;
    
    let response = PasswordMsisdnRequestTokenResponse {
        sid: session.session_id,
        submit_url: Some(format!(
            "{}/_matrix/client/v3/account/password/msisdn/submit_token",
            state.homeserver_name
        )),
    };
    
    info!("Password reset SMS verification session created with ID: {}", response.sid);
    Ok(Json(response))
}

/// Generate 6-digit numeric verification code for SMS
fn generate_verification_code() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    format!("{:06}", rng.random_range(0..1000000))
}

/// Send password reset SMS
async fn send_password_reset_sms(
    phone: &str,
    code: &str,
    state: &AppState,
) -> Result<(), Response> {
    let config = &state.config.sms_config;
    
    if !config.enabled {
        error!("SMS service unavailable - password reset cannot proceed");
        return Err(MatrixError::Unknown.into_response());
    }
    
    let message = format!(
        "Your Matrix password reset code is: {}. This code will expire in 10 minutes. Do not share this code.",
        code
    );
    
    match config.provider.as_str() {
        "twilio" => send_twilio_sms(phone, &message, config, &state.http_client).await
            .map_err(|_| MatrixError::Unknown.into_response()),
        _ => {
            error!("Unsupported SMS provider: {}", config.provider);
            Err(MatrixError::Unknown.into_response())
        },
    }
}

/// Send SMS via Twilio API
async fn send_twilio_sms(
    to: &str,
    message: &str,
    config: &crate::config::server_config::SmsConfig,
    client: &reqwest::Client,
) -> Result<(), axum::http::StatusCode> {
    use base64::{Engine, engine::general_purpose};
    
    let url = format!(
        "{}/2010-04-01/Accounts/{}/Messages.json",
        config.api_base_url,
        config.api_key
    );
    
    let params = [
        ("To", to),
        ("From", &config.from_number),
        ("Body", message),
    ];
    
    let auth_header = format!(
        "Basic {}",
        general_purpose::STANDARD.encode(format!("{}:{}", config.api_key, config.api_secret))
    );
    
    let response = client
        .post(&url)
        .header("Authorization", auth_header)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .map_err(|e| {
            error!("Failed to send Twilio SMS request: {:?}", e);
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        })?;
    
    if response.status().is_success() {
        info!("Password reset SMS sent successfully to: {}", to);
        Ok(())
    } else {
        let status = response.status();
        match response.text().await {
            Ok(error_text) => {
                error!("Twilio SMS failed with status {}: {}", status, error_text);
            }
            Err(e) => {
                error!("Twilio SMS failed with status {} (unable to read error body: {:?})", status, e);
            }
        }
        Err(axum::http::StatusCode::SERVICE_UNAVAILABLE)
    }
}
