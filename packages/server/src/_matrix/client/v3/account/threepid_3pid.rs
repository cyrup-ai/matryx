use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use rand;
use rand::distr::Alphanumeric;
use rand::{Rng, rng};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use subtle::ConstantTimeEq;
use tracing::{error, info, warn};

use crate::AppState;
use crate::auth::uia::{UiaAuth, UiaFlow};
use matryx_entity::types::third_party_validation_session::ThirdPartyValidationSession;
use matryx_surrealdb::repository::ProfileManagementService;
use matryx_surrealdb::repository::third_party_validation_session::{
    ThirdPartyValidationSessionRepository,
    ThirdPartyValidationSessionRepositoryTrait,
};

#[derive(Serialize)]
pub struct ThreePid {
    pub medium: String,
    pub address: String,
    pub validated_at: u64,
    pub added_at: u64,
}

// Using the proper entity from matryx_entity
// ThirdPartyValidationSession is imported above

#[derive(Serialize)]
pub struct ThreePidsResponse {
    pub threepids: Vec<ThreePid>,
}

#[derive(Deserialize)]
pub struct AddThreePidRequest {
    pub client_secret: String,
    pub sid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Value>,
}

#[derive(Deserialize)]
pub struct VerifyTokenRequest {
    pub token: String,
    pub session_id: String,
    pub client_secret: String,
}

#[derive(Deserialize)]
pub struct RequestTokenRequest {
    pub client_secret: String,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub send_attempt: u32,
}

pub async fn get_threepids(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ThreePidsResponse>, StatusCode> {
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

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Get user's third-party identifiers using ProfileManagementService
    let threepids = match profile_service
        .manage_third_party_ids(
            &token_info.user_id,
            matryx_entity::types::ThirdPartyOperation::List,
        )
        .await
    {
        Ok(response) => {
            response
                .threepids
                .into_iter()
                .map(|tp| {
                    ThreePid {
                        medium: tp.medium,
                        address: tp.address,
                        validated_at: tp.validated_at.unwrap_or(0) as u64,
                        added_at: tp.added_at as u64,
                    }
                })
                .collect()
        },
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    Ok(Json(ThreePidsResponse { threepids }))
}

pub async fn add_threepid(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AddThreePidRequest>,
) -> Result<Json<Value>, StatusCode> {
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

    // Validate auth data if provided (UIA - User Interactive Authentication)
    if let Some(auth_data) = &request.auth {
        // Implement proper UIA validation according to Matrix specification
        info!("Processing UIA authentication for 3PID addition");

        // Parse auth data as UIA authentication
        let uia_auth: UiaAuth = serde_json::from_value(auth_data.clone())
            .map_err(|e| {
                error!("Failed to parse UIA auth data: {:?}", e);
                StatusCode::BAD_REQUEST
            })?;

        // Use centralized UIA service from AppState
        let uia_service = &state.uia_service;

        // Validate the authentication
        let session_id = match &uia_auth.session {
            Some(id) if !id.is_empty() => id.clone(),
            _ => {
                warn!("Invalid or missing session ID in UIA auth");
                return Ok(Json(json!({
                    "errcode": "M_INVALID_PARAM",
                    "error": "Invalid or missing session ID"
                })));
            }
        };
        match uia_service.process_auth(&session_id, uia_auth).await {
            Ok(_) => {
                info!("UIA authentication successful for 3PID addition");
                // Authentication passed, continue with 3PID addition
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

        // Define required authentication flows for 3PID operations
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
            Some(&token_info.user_id),
            None, // device_id not required for 3PID operations
            flows.clone(),
            std::collections::HashMap::new(),
        ).await.map_err(|e| {
            error!("Failed to start UIA session: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        // Return UIA challenge per Matrix spec
        return Ok(Json(json!({
            "flows": flows,
            "params": {},
            "session": session.session_id,
            "completed": [],
            "error": "User Interactive Authentication required",
            "errcode": "M_FORBIDDEN"
        })));
    }

    // Validate session and get verified 3PID
    let session = validate_3pid_session(&request.sid, &request.client_secret, &state).await?;

    // Associate 3PID with user account
    associate_3pid_with_account(&token_info.user_id, &session, &state).await?;

    // Clean up session
    let repo = ThirdPartyValidationSessionRepository::new(state.db.clone());
    repo.delete_session(&request.sid).await.map_err(|e| {
        error!("Failed to cleanup session: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("3PID successfully added to account: {} -> {}", session.address, token_info.user_id);
    Ok(Json(json!({})))
}

/// Generate secure verification token
fn generate_verification_token() -> String {
    let mut rng = rng();
    (0..32).map(|_| rng.sample(Alphanumeric) as char).collect()
}

/// Generate numeric verification code for SMS
fn generate_verification_code() -> String {
    let mut rng = rng();
    format!("{:06}", rng.random_range(100000..999999))
}

/// Timing-safe string comparison
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}

/// Create a new 3PID validation session
async fn create_3pid_session(
    medium: &str,
    address: &str,
    client_secret: &str,
    state: &AppState,
) -> Result<ThirdPartyValidationSession, StatusCode> {

    // Generate unique session ID
    let session_id = uuid::Uuid::new_v4().to_string();

    // Generate verification token based on medium type
    let verification_token = match medium {
        "email" => generate_verification_token(), // Long secure token for email
        "msisdn" => generate_verification_code(), // Short numeric code for SMS
        _ => generate_verification_token(), // Default to secure token
    };

    // Session expires in 1 hour
    let expires_at = chrono::Utc::now().timestamp() + 3600;

    // Create session
    let session = ThirdPartyValidationSession::new(
        session_id,
        client_secret.to_string(),
        medium.to_string(),
        address.to_string(),
        verification_token.clone(),
        expires_at,
    );

    // Save to database
    let repo = ThirdPartyValidationSessionRepository::new(state.db.clone());
    repo.create_session(&session).await.map_err(|e| {
        error!("Failed to create 3PID session: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Send verification message based on medium
    match medium {
        "email" => {
            send_verification_email(address, &verification_token, state).await?;
        },
        "msisdn" => {
            let verification_code = generate_verification_code();
            send_verification_sms(address, &verification_code, state).await?;
        },
        _ => {
            error!("Unsupported 3PID medium: {}", medium);
            return Err(StatusCode::BAD_REQUEST);
        },
    }

    info!(
        "Created 3PID validation session {} for {} address: {}",
        session.session_id, medium, address
    );
    Ok(session)
}

/// Validate 3PID session
async fn validate_3pid_session(
    session_id: &str,
    client_secret: &str,
    state: &AppState,
) -> Result<ThirdPartyValidationSession, StatusCode> {
    let repo = ThirdPartyValidationSessionRepository::new(state.db.clone());

    let session = repo
        .get_session_by_id_and_secret(session_id, client_secret)
        .await
        .map_err(|e| {
            error!("Failed to query 3PID session: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("3PID session not found: {}", session_id);
            StatusCode::BAD_REQUEST
        })?;

    // Check expiration
    if session.is_expired() {
        warn!("Expired 3PID session: {}", session_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if verified
    if !session.verified {
        warn!("Unverified 3PID session: {}", session_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok(session)
}

/// Associate verified 3PID with user account
async fn associate_3pid_with_account(
    user_id: &str,
    session: &ThirdPartyValidationSession,
    state: &AppState,
) -> Result<(), StatusCode> {
    let profile_service = ProfileManagementService::new(state.db.clone());

    // Add 3PID to user account using ProfileManagementService
    let operation = matryx_entity::types::ThirdPartyOperation::Add {
        medium: session.medium.clone(),
        address: session.address.clone(),
        validated: true,
    };

    match profile_service.manage_third_party_ids(user_id, operation).await {
        Ok(_) => {
            info!("3PID associated with account: {} -> {}", session.address, user_id);
            Ok(())
        },
        Err(_) => {
            error!("Failed to associate 3PID with account: {}", session.address);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
    }
}

/// Send verification email with SMTP
async fn send_verification_email(
    email: &str,
    token: &str,
    state: &AppState,
) -> Result<(), StatusCode> {
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{Message, SmtpTransport, Transport};

    let config = &state.config.email_config;

    if !config.enabled {
        warn!("Email verification disabled - cannot send verification email");
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let verification_url = format!(
        "https://{}/_matrix/client/v3/account/3pid/email/verify?token={}",
        state.config.homeserver_name, token
    );

    let email_body = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <title>Verify your Matrix email address</title>
</head>
<body style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h2 style="color: #333;">Verify your email address</h2>
    <p>You've requested to add this email address to your Matrix account on <strong>{}</strong>.</p>
    <p>Click the button below to verify your email address:</p>
    <div style="text-align: center; margin: 30px 0;">
        <a href="{}" style="background-color: #007bff; color: white; padding: 12px 24px; text-decoration: none; border-radius: 4px; display: inline-block;">Verify Email Address</a>
    </div>
    <p>Or copy and paste this link into your browser:</p>
    <p style="word-break: break-all; background-color: #f8f9fa; padding: 10px; border-radius: 4px;">{}</p>
    <p style="color: #666; font-size: 14px;">If you didn't request this verification, you can safely ignore this email.</p>
    <p style="color: #666; font-size: 14px;">This link will expire in 1 hour.</p>
</body>
</html>"#,
        state.config.homeserver_name, verification_url, verification_url
    );

    let email_message = Message::builder()
        .from(config.from_address.parse().map_err(|e| {
            error!("Invalid from email address: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?)
        .to(email.parse().map_err(|e| {
            error!("Invalid recipient email address: {:?}", e);
            StatusCode::BAD_REQUEST
        })?)
        .subject("Verify your Matrix email address")
        .header(lettre::message::header::ContentType::TEXT_HTML)
        .body(email_body)
        .map_err(|e| {
            error!("Failed to build email message: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Configure SMTP transport
    let creds = Credentials::new(config.smtp_username.clone(), config.smtp_password.clone());

    let mailer = SmtpTransport::relay(&config.smtp_server)
        .map_err(|e| {
            error!("Failed to create SMTP transport: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .port(config.smtp_port)
        .credentials(creds)
        .build();

    // Send email
    tokio::task::spawn_blocking(move || mailer.send(&email_message))
        .await
        .map_err(|e| {
            error!("Failed to spawn email sending task: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map_err(|e| {
            error!("Failed to send verification email: {:?}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    info!("Verification email sent successfully to: {}", email);
    Ok(())
}

/// Send verification SMS with Twilio
async fn send_verification_sms(
    phone: &str,
    code: &str,
    state: &AppState,
) -> Result<(), StatusCode> {
    let config = &state.config.sms_config;

    if !config.enabled {
        warn!("SMS verification disabled - cannot send verification SMS");
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let message = format!(
        "Your Matrix verification code is: {}. This code will expire in 10 minutes. Do not share this code with anyone.",
        code
    );

    match config.provider.as_str() {
        "twilio" => send_twilio_sms(phone, &message, config, &state.http_client).await,
        _ => {
            error!("Unsupported SMS provider: {}", config.provider);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
    }
}

/// Send SMS via Twilio API
async fn send_twilio_sms(
    to: &str,
    message: &str,
    config: &crate::config::server_config::SmsConfig,
    client: &reqwest::Client,
) -> Result<(), StatusCode> {
    use base64::{Engine, engine::general_purpose};

    let url =
        format!("https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json", config.api_key);

    let params = [("To", to), ("From", &config.from_number), ("Body", message)];

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
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    if response.status().is_success() {
        info!("SMS sent successfully to: {}", to);
        Ok(())
    } else {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        error!("Twilio SMS failed with status {}: {}", status, error_text);
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

/// Verify 3PID token endpoint
pub async fn verify_3pid_token(
    State(state): State<AppState>,
    Json(request): Json<VerifyTokenRequest>,
) -> Result<Json<Value>, StatusCode> {
    let repo = ThirdPartyValidationSessionRepository::new(state.db.clone());

    // Look up session
    let session = repo
        .get_session_by_id_and_secret(&request.session_id, &request.client_secret)
        .await
        .map_err(|e| {
            error!("Failed to query 3PID session: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("3PID session not found: {}", request.session_id);
            StatusCode::BAD_REQUEST
        })?;

    // Check expiration
    if session.is_expired() {
        warn!("Expired 3PID session: {}", request.session_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if already verified
    if session.verified {
        info!("3PID session already verified: {}", request.session_id);
        return Ok(Json(json!({ "success": true })));
    }

    // Check max attempts
    if session.max_attempts_reached() {
        warn!("Max verification attempts reached for session: {}", request.session_id);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Increment attempt count
    repo.increment_session_attempts(&request.session_id).await.map_err(|e| {
        error!("Failed to increment session attempts: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Validate token with timing-safe comparison
    if !constant_time_eq(session.verification_token.as_bytes(), request.token.as_bytes()) {
        warn!("Invalid verification token for session: {}", request.session_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Mark session as verified
    repo.mark_session_verified(&request.session_id).await.map_err(|e| {
        error!("Failed to mark session as verified: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("3PID token verified successfully for session: {}", request.session_id);
    Ok(Json(json!({ "success": true })))
}

/// Request 3PID validation token endpoint
pub async fn request_3pid_token(
    State(state): State<AppState>,
    Json(request): Json<RequestTokenRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Validate request - must have either email or phone_number
    let (medium, address) = if let Some(email) = &request.email {
        ("email", email.as_str())
    } else if let Some(phone) = &request.phone_number {
        ("msisdn", phone.as_str())
    } else {
        warn!("Request token request missing both email and phone_number");
        return Err(StatusCode::BAD_REQUEST);
    };

    // Basic validation
    if medium == "email" && !address.contains('@') {
        warn!("Invalid email address format: {}", address);
        return Err(StatusCode::BAD_REQUEST);
    }

    if medium == "msisdn" && !address.starts_with('+') {
        warn!("Phone number must be in international format starting with +");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check for existing active sessions for this address
    let repo = ThirdPartyValidationSessionRepository::new(state.db.clone());
    let existing_sessions = repo.get_sessions_by_address(medium, address).await.map_err(|e| {
        error!("Failed to check existing sessions: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Validate send_attempt - used to prevent replay attacks (Matrix spec requirement)
    if request.send_attempt == 0 {
        warn!("Invalid send_attempt value: must be positive integer");
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check rate limiting - only allow one active session per address
    let active_sessions: Vec<_> = existing_sessions
        .into_iter()
        .filter(|s| !s.is_expired() && s.is_valid_for_verification())
        .collect();

    if !active_sessions.is_empty() {
        warn!("Active validation session already exists for address: {}", address);
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Log send_attempt for audit trail (Matrix spec compliance)
    info!(
        "3PID token request - Medium: {}, Address: {}, Send attempt: {}, Client secret: {}",
        medium, address, request.send_attempt, request.client_secret
    );

    // Create new validation session
    let session = create_3pid_session(medium, address, &request.client_secret, &state).await?;

    info!("Created 3PID validation session: {} for {}", session.session_id, address);

    Ok(Json(json!({
        "sid": session.session_id,
        "submit_url": format!("https://{}/_matrix/client/v3/account/3pid/verify", state.config.homeserver_name)
    })))
}
