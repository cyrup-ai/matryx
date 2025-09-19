use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use bcrypt::{hash, DEFAULT_COST, BcryptError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info, warn};

use crate::AppState;
use matryx_entity::types::{Device, Session, User};
use matryx_surrealdb::repository::{
    DeviceRepository,
    RepositoryError,
    SessionRepository,
    UserRepository,
};

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
    pub expires_in_ms: Option<u64>,

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
/// Implements Matrix Client-Server API user registration endpoint with complete user creation.
///
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
    info!("Processing user registration request");

    // Validate registration request
    let username = validate_registration_request(&request)?;

    // Generate Matrix user ID
    let user_id = format!("@{}:{}", username, state.homeserver_name);

    info!("Registering new user: {}", user_id);

    // Initialize repositories for database operations
    let user_repo = UserRepository::new(state.db.clone());
    let device_repo = DeviceRepository::new(state.db.clone());
    let session_repo = SessionRepository::new(state.db.clone());

    // Check if user already exists
    if user_exists(&user_repo, &user_id).await? {
        warn!("Registration failed - user already exists: {}", user_id);
        return Err(RegistrationError::UserInUse.into());
    }

    // Create user account
    let user = create_user_account(&user_repo, &user_id, &request).await?;

    // If inhibit_login is true, return early without creating session
    if request.inhibit_login {
        info!("User registration completed without login for: {}", user_id);
        return Ok(Json(RegistrationResponse {
            user_id,
            access_token: None,
            device_id: None,
            refresh_token: None,
            expires_in_ms: None,
            well_known: None,
        }));
    }

    // Generate device ID if not provided
    let device_id = request.device_id.unwrap_or_else(|| generate_device_id());

    // Create device for the user
    let device = create_user_device(
        &device_repo,
        &user_id,
        &device_id,
        &request.initial_device_display_name,
        &headers,
    )
    .await?;

    // Generate access and refresh tokens
    let access_token = generate_access_token();
    let refresh_token = if request.refresh_token {
        Some(generate_refresh_token())
    } else {
        None
    };

    // Create user session
    let session = create_user_session(
        &session_repo,
        &user_id,
        &device_id,
        &access_token,
        &refresh_token,
        &headers,
    )
    .await?;

    // Register session with authentication service for JWT generation
    let jwt_token = register_session_with_auth_service(
        &state.session_service,
        &user_id,
        &device_id,
        &access_token,
    )?;

    info!("User registration completed successfully for: {}", user_id);

    // Return registration response
    Ok(Json(RegistrationResponse {
        user_id,
        access_token: Some(jwt_token),
        device_id: Some(device.device_id),
        refresh_token,
        expires_in_ms: Some(3600000), // 1 hour in milliseconds
        well_known: Some(serde_json::json!({
            "m.homeserver": {
                "base_url": format!("https://{}", state.homeserver_name)
            }
        })),
    }))
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
    if let Some(password) = &request.password {
        if !is_strong_password(password) {
            warn!("Weak password provided for user: {}", username);
            return Err(RegistrationError::WeakPassword);
        }
    }

    Ok(username)
}

/// Check if user already exists in database
async fn user_exists(user_repo: &UserRepository, user_id: &str) -> Result<bool, RegistrationError> {
    match user_repo.get_by_id(user_id).await {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(e) => {
            error!("Database error checking user existence: {}", e);
            Err(RegistrationError::DatabaseError)
        },
    }
}

/// Create user account in database
async fn create_user_account(
    user_repo: &UserRepository,
    user_id: &str,
    request: &RegistrationRequest,
) -> Result<User, RegistrationError> {
    let password_hash = match &request.password {
        Some(password) => {
            match hash_password(password) {
                Ok(hash) => Some(hash),
                Err(e) => {
                    error!("Password hashing failed: {:?}", e);
                    return Err(RegistrationError::InternalError);
                }
            }
        },
        None => None,
    };

    let user = User {
        user_id: user_id.to_string(),
        display_name: None,
        avatar_url: None,
        password_hash: password_hash.unwrap_or_else(|| "no_password".to_string()),
        created_at: chrono::Utc::now(),
        last_seen: None,
        is_active: true,
        is_admin: false,
        account_data: None,
    };

    match user_repo.create(&user).await {
        Ok(created_user) => {
            info!("User account created: {}", user_id);
            Ok(created_user)
        },
        Err(e) => {
            error!("Failed to create user account {}: {}", user_id, e);
            Err(RegistrationError::DatabaseError)
        },
    }
}

/// Create device for the user
async fn create_user_device(
    device_repo: &DeviceRepository,
    user_id: &str,
    device_id: &str,
    display_name: &Option<String>,
    headers: &HeaderMap,
) -> Result<Device, RegistrationError> {
    let user_agent = extract_user_agent(headers);
    let client_ip = extract_client_ip(headers);

    let device = Device {
        device_id: device_id.to_string(),
        user_id: user_id.to_string(),
        display_name: display_name.clone(),
        last_seen_ip: Some(client_ip.clone()),
        last_seen_ts: Some(chrono::Utc::now().timestamp_millis()),
        created_at: chrono::Utc::now(),
        hidden: Some(false),
        device_keys: None,
        one_time_keys: None,
        fallback_keys: None,
        user_agent: Some(user_agent),
        initial_device_display_name: display_name.clone(),
    };

    match device_repo.create(&device).await {
        Ok(created_device) => {
            info!("Device created for user {}: {}", user_id, device_id);
            Ok(created_device)
        },
        Err(e) => {
            error!("Failed to create device {} for user {}: {}", device_id, user_id, e);
            Err(RegistrationError::DatabaseError)
        },
    }
}

/// Create user session in database
async fn create_user_session(
    session_repo: &SessionRepository,
    user_id: &str,
    device_id: &str,
    access_token: &str,
    refresh_token: &Option<String>,
    headers: &HeaderMap,
) -> Result<Session, RegistrationError> {
    let client_ip = extract_client_ip(headers);
    let user_agent = extract_user_agent(headers);

    let session = Session {
        session_id: format!("{}:{}", user_id, device_id),
        user_id: user_id.to_string(),
        device_id: device_id.to_string(),
        access_token: access_token.to_string(),
        refresh_token: refresh_token.clone(),
        created_at: chrono::Utc::now(),
        expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        last_seen: Some(chrono::Utc::now()),
        last_used_at: Some(chrono::Utc::now()),
        last_used_ip: Some(client_ip),
        user_agent: Some(user_agent),
        is_active: true,
        valid: true,
        puppets_user_id: None,
    };

    match session_repo.create(&session).await {
        Ok(created_session) => {
            info!("Session created for user {}: {}", user_id, device_id);
            Ok(created_session)
        },
        Err(e) => {
            error!("Failed to create session for user {}: {}", user_id, e);
            Err(RegistrationError::DatabaseError)
        },
    }
}

/// Register session with authentication service for JWT generation
fn register_session_with_auth_service(
    session_service: &crate::auth::MatrixSessionService,
    user_id: &str,
    device_id: &str,
    access_token: &str,
) -> Result<String, RegistrationError> {
    match session_service.create_user_token(user_id, device_id, access_token, None, 3600) {
        Ok(jwt_token) => {
            info!("JWT token created for user: {}", user_id);
            Ok(jwt_token)
        },
        Err(e) => {
            error!("Failed to create JWT token for user {}: {}", user_id, e);
            Err(RegistrationError::InternalError)
        },
    }
}

/// Utility functions
fn is_valid_username(username: &str) -> bool {
    // Matrix username validation: lowercase letters, numbers, hyphens, underscores, periods
    username
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.') &&
        !username.is_empty() &&
        username.len() <= 255
}

fn is_strong_password(password: &str) -> bool {
    // Basic password strength: at least 8 characters
    password.len() >= 8
}

fn generate_username() -> String {
    format!("user_{}", chrono::Utc::now().timestamp_millis())
}

fn generate_device_id() -> String {
    format!("DEVICE_{}", chrono::Utc::now().timestamp_millis())
}

fn generate_access_token() -> String {
    format!("syt_{}", chrono::Utc::now().timestamp_millis())
}

fn generate_refresh_token() -> String {
    format!("syr_{}", chrono::Utc::now().timestamp_millis())
}

fn hash_password(password: &str) -> Result<String, BcryptError> {
    hash(password, DEFAULT_COST)
}

fn extract_user_agent(headers: &HeaderMap) -> String {
    headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("Unknown")
        .to_string()
}

fn extract_client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim())
        .unwrap_or("127.0.0.1")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_username_validation() {
        assert!(is_valid_username("alice"));
        assert!(is_valid_username("bob123"));
        assert!(is_valid_username("user_name"));
        assert!(is_valid_username("test-user"));
        assert!(is_valid_username("user.name"));

        assert!(!is_valid_username("Alice")); // uppercase
        assert!(!is_valid_username("user@domain")); // invalid chars
        assert!(!is_valid_username("")); // empty
    }

    #[test]
    fn test_password_strength() {
        assert!(is_strong_password("password123"));
        assert!(is_strong_password("12345678"));

        assert!(!is_strong_password("1234567")); // too short
        assert!(!is_strong_password("")); // empty
    }

    #[test]
    fn test_token_generation() {
        let access_token = generate_access_token();
        assert!(access_token.starts_with("syt_"));

        let refresh_token = generate_refresh_token();
        assert!(refresh_token.starts_with("syr_"));

        let device_id = generate_device_id();
        assert!(device_id.starts_with("DEVICE_"));
    }
}
