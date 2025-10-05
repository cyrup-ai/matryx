use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info, warn};

use crate::AppState;
use matryx_surrealdb::repository::InfrastructureService;

/// Matrix Client-Server API Registration Request
#[derive(Debug, Deserialize)]
pub struct RegistrationRequest {
    /// The basis for the localpart of the desired Matrix ID.
    /// If omitted, the homeserver MUST generate a Matrix ID local part.
    pub username: Option<String>,

    /// The desired password for the account.
    pub password: Option<String>,

    /// ID of the client device. If this does not correspond to a known client device,
    /// a new device will be created.
    pub device_id: Option<String>,

    /// A display name to assign to the newly-created device.
    pub initial_device_display_name: Option<String>,

    /// If true, an access_token and device_id should not be returned from this call,
    /// therefore preventing an automatic login.
    #[serde(default)]
    pub inhibit_login: bool,

    /// Whether the client supports refresh tokens.
    #[serde(default)]
    pub refresh_token: bool,

    /// Additional authentication information for the user-interactive authentication API.
    pub auth: Option<Value>,
}

/// Matrix Client-Server API Registration Response
#[derive(Debug, Serialize)]
pub struct RegistrationResponse {
    /// The fully-qualified Matrix user ID (MXID) that has been registered.
    pub user_id: String,

    /// An access token for the account. This access token can then be used to
    /// authorize other requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,

    /// ID of the registered device. Will be the same as the corresponding parameter
    /// in the request, if one was specified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// A refresh_token may be exchanged for a new access_token using the /tokenrefresh API endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// The lifetime of the access token, in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_ms: Option<i64>,

    /// Optional client configuration provided by the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub well_known: Option<Value>,
}

#[derive(Debug)]
pub enum RegistrationError {
    InvalidUsername,
    UserInUse,
    WeakPassword,
    DatabaseError,
    InternalError,
}

impl From<RegistrationError> for StatusCode {
    fn from(error: RegistrationError) -> Self {
        match error {
            RegistrationError::InvalidUsername => StatusCode::BAD_REQUEST,
            RegistrationError::UserInUse => StatusCode::BAD_REQUEST,
            RegistrationError::WeakPassword => StatusCode::BAD_REQUEST,
            RegistrationError::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
            RegistrationError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// POST /_matrix/client/v3/register
///
/// Implements Matrix Client-Server API user registration endpoint using InfrastructureService.
/// Features:
/// - User account creation with password hashing
/// - Device registration and management
/// - Session creation and JWT token generation
/// - Matrix specification compliance
/// - Comprehensive error handling
pub async fn post_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegistrationRequest>,
) -> Result<Json<RegistrationResponse>, StatusCode> {
    // Extract client information for audit logging
    let client_ip = extract_client_ip(&headers);
    let user_agent = extract_user_agent(&headers);

    info!(
        "Processing user registration request from IP: {}, User-Agent: {}",
        client_ip, user_agent
    );

    // Handle User Interactive Authentication (UIA) if required
    if let Some(auth_data) = &request.auth {
        info!("Processing UIA authentication data for registration");

        // Validate auth data format
        if !auth_data.is_object() {
            error!("Invalid UIA auth data format");
            return Err(StatusCode::BAD_REQUEST);
        }

        // Check if CAPTCHA is required for registration from this IP/client
        let captcha_service = crate::auth::CaptchaService::new(
            matryx_surrealdb::repository::CaptchaRepository::new(state.db.clone()),
            crate::auth::captcha::CaptchaConfig::from_env(),
        );

        if captcha_service
            .is_captcha_required(&client_ip, "registration")
            .await
            .unwrap_or(false)
        {
            // Check if CAPTCHA challenge is provided in auth data
            if let Some(captcha_response) = auth_data.get("response") {
                if let Some(challenge_id) = auth_data.get("session") {
                    // Create verification request
                    let verification_request = crate::auth::captcha::CaptchaVerificationRequest {
                        challenge_id: challenge_id.as_str().unwrap_or("").to_string(),
                        response: captcha_response.as_str().unwrap_or("").to_string(),
                        remote_ip: Some(client_ip.clone()),
                    };

                    // Validate CAPTCHA response
                    match captcha_service.verify_captcha(verification_request).await {
                        Ok(response) => {
                            if response.success {
                                info!("CAPTCHA validation successful for registration");
                            } else {
                                warn!(
                                    "CAPTCHA validation failed for registration from IP: {}",
                                    client_ip
                                );
                                return Err(StatusCode::UNAUTHORIZED);
                            }
                        },
                        Err(e) => {
                            error!("CAPTCHA validation error: {:?}", e);
                            return Err(StatusCode::BAD_REQUEST);
                        },
                    }
                } else {
                    error!("Missing CAPTCHA session in auth data");
                    return Err(StatusCode::BAD_REQUEST);
                }
            } else {
                // CAPTCHA required but not provided - return challenge
                match captcha_service
                    .create_challenge(Some(client_ip.clone()), Some(user_agent.clone()), None)
                    .await
                {
                    Ok(_challenge) => {
                        info!("Created CAPTCHA challenge for registration");
                        // Return UIA flow indicating CAPTCHA is required
                        return Err(StatusCode::UNAUTHORIZED); // Matrix spec: 401 with flows
                    },
                    Err(e) => {
                        error!("Failed to create CAPTCHA challenge: {:?}", e);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    },
                }
            }
        }
    }

    // Validate registration request
    let username = validate_registration_request(&request)?;

    info!("Registering new user: {}", username);

    // Create InfrastructureService instance
    let infrastructure_service = create_infrastructure_service(&state).await;

    // If inhibit_login is true, just register without creating session
    if request.inhibit_login {
        // Register user without login using InfrastructureService
        let user_id = format!("@{}:{}", username, state.homeserver_name);
        let password = request.password.as_deref().unwrap_or("no_password");

        match infrastructure_service
            .register_new_user(
                &username,
                password,
                None,
                request.initial_device_display_name.as_deref(),
            )
            .await
        {
            Ok(_) => {
                info!("User registration completed without login for: {}", user_id);
                return Ok(Json(RegistrationResponse {
                    user_id,
                    access_token: None,
                    device_id: None,
                    refresh_token: None,
                    expires_in_ms: None,
                    well_known: None,
                }));
            },
            Err(e) => {
                error!("Failed to register user {}: {:?}", username, e);
                return Err(RegistrationError::DatabaseError.into());
            },
        }
    }

    // Register user with device using InfrastructureService
    let password = request.password.as_deref().unwrap_or("no_password");
    let device_id = request.device_id.as_deref();

    match infrastructure_service
        .register_new_user_with_options(
            &username,
            password,
            device_id,
            request.initial_device_display_name.as_deref(),
            request.refresh_token, // Pass refresh token preference
        )
        .await
    {
        Ok(registration_result) => {
            info!("User registration completed successfully for: {}", registration_result.user_id);

            // Return registration response with refresh token if requested and supported
            let refresh_token = if request.refresh_token {
                registration_result.refresh_token
            } else {
                None
            };

            Ok(Json(RegistrationResponse {
                user_id: registration_result.user_id,
                access_token: Some(registration_result.access_token),
                device_id: Some(registration_result.device_id),
                refresh_token,
                expires_in_ms: registration_result.expires_in_ms,
                well_known: Some(serde_json::json!({
                    "m.homeserver": {
                        "base_url": format!("https://{}", state.homeserver_name)
                    }
                })),
            }))
        },
        Err(e) => {
            error!("Failed to register user {}: {:?}", username, e);
            Err(RegistrationError::DatabaseError.into())
        },
    }
}

async fn create_infrastructure_service(
    state: &AppState,
) -> InfrastructureService<surrealdb::engine::any::Any> {
    let websocket_repo = matryx_surrealdb::repository::WebSocketRepository::new(state.db.clone());
    let transaction_repo =
        matryx_surrealdb::repository::TransactionRepository::new(state.db.clone());
    let key_server_repo = matryx_surrealdb::repository::KeyServerRepository::new(state.db.clone());
    let registration_repo =
        matryx_surrealdb::repository::RegistrationRepository::new(state.db.clone());
    let directory_repo = matryx_surrealdb::repository::DirectoryRepository::new(state.db.clone());
    let device_repo = matryx_surrealdb::repository::DeviceRepository::new(state.db.clone());
    let auth_repo = matryx_surrealdb::repository::AuthRepository::new(state.db.clone());

    InfrastructureService::new(
        websocket_repo,
        transaction_repo,
        key_server_repo,
        registration_repo,
        directory_repo,
        device_repo,
        auth_repo,
    )
}

/// Validate registration request parameters
fn validate_registration_request(
    request: &RegistrationRequest,
) -> Result<String, RegistrationError> {
    // Generate username if not provided
    let username = match &request.username {
        Some(username) => {
            // Validate username format
            if !is_valid_username(username) {
                warn!("Invalid username format: {}", username);
                return Err(RegistrationError::InvalidUsername);
            }
            username.clone()
        },
        None => generate_username(),
    };

    // Validate password strength if provided
    if let Some(password) = &request.password
        && !is_strong_password(password)
    {
        warn!("Weak password provided for user: {}", username);
        return Err(RegistrationError::WeakPassword);
    }

    Ok(username)
}

/// Validate username format according to Matrix specification
fn is_valid_username(username: &str) -> bool {
    // Basic validation - Matrix usernames should be alphanumeric with some special chars
    !username.is_empty()
        && username.len() <= 255
        && username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
        && !username.starts_with('_')
}

/// Check password strength
fn is_strong_password(password: &str) -> bool {
    // Basic password strength check
    password.len() >= 8
}

/// Generate a random username
fn generate_username() -> String {
    format!("user_{}", uuid::Uuid::new_v4().to_string().replace('-', "")[..8].to_lowercase())
}

/// Extract client IP from headers
fn extract_client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// Extract user agent from headers
fn extract_user_agent(headers: &HeaderMap) -> String {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
