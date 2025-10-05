use crate::utils::session_helpers::create_secure_session_cookie;
use axum::http::HeaderMap;
use axum::{Json, extract::ConnectInfo, extract::State, http::StatusCode};
use regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::SocketAddr;
use tower_cookies::Cookies;
use tracing::{error, info, warn};

use crate::auth::captcha::CaptchaService;
use crate::state::AppState;
// use crate::auth::errors::MatrixAuthError; // TODO: Use for proper error handling
use crate::auth::refresh_token::TokenPair;
// Cookie helper function is in main.rs
use matryx_surrealdb::repository::{
    ApplicationService, AuthRepository, DeviceRepository, SsoUserInfo, captcha::CaptchaRepository,
};
use std::sync::Arc;

pub mod get_token;
pub mod password;
pub mod sso;

#[derive(Deserialize)]
pub struct LoginRequest {
    #[serde(rename = "type")]
    pub login_type: String,
    pub user: Option<String>,
    pub password: Option<String>,
    pub device_id: Option<String>,
    pub initial_device_display_name: Option<String>,
    pub refresh_token: Option<String>,
    pub token: Option<String>,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub well_known: Option<Value>,
}

#[derive(Serialize)]
pub struct LoginFlow {
    #[serde(rename = "type")]
    pub flow_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_providers: Option<Vec<Value>>,
}

#[derive(Serialize)]
pub struct LoginFlowsResponse {
    flows: Vec<LoginFlow>,
}

/// GET /_matrix/client/v3/login
pub async fn get(State(state): State<AppState>) -> Result<Json<LoginFlowsResponse>, StatusCode> {
    // Build available login flows based on server configuration
    let mut flows = vec![LoginFlow {
        flow_type: "m.login.password".to_string(),
        identity_providers: None,
    }];

    // Add token login flow for application service tokens and SSO tokens
    flows.push(LoginFlow {
        flow_type: "m.login.token".to_string(),
        identity_providers: None,
    });

    // Add SSO flows if configured identity providers are available
    let sso_providers = get_configured_sso_providers(&state).await;
    if !sso_providers.is_empty() {
        flows.push(LoginFlow {
            flow_type: "m.login.sso".to_string(),
            identity_providers: Some(sso_providers),
        });
    }

    Ok(Json(LoginFlowsResponse { flows }))
}

/// POST /_matrix/client/v3/login
///
/// Handles Matrix login requests by delegating to appropriate authentication modules
/// based on the login type specified in the request.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    cookies: Cookies,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    let result = match request.login_type.as_str() {
        "m.login.password" => {
            handle_password_login(state, addr, headers, cookies.clone(), request).await
        },
        "m.login.token" => handle_token_login(state, addr, headers, request).await,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    // Set secure session cookie for OAuth2 integration upon successful login
    if let Ok(login_response) = &result {
        let session_cookie =
            create_secure_session_cookie("matrix_session", &login_response.access_token);
        cookies.add(session_cookie);
    }

    result
}

async fn handle_password_login(
    state: AppState,
    addr: SocketAddr,
    headers: HeaderMap,
    cookies: Cookies,
    request: LoginRequest,
) -> Result<Json<LoginResponse>, StatusCode> {
    // Handle refresh token login if present
    if let Some(refresh_token) = request.refresh_token {
        return handle_refresh_token_login(state, refresh_token, request.device_id).await;
    }

    // Handle SSO token login if present
    if let Some(sso_token) = request.token {
        return handle_sso_token_login(state, sso_token, request.device_id).await;
    }

    // Validate required fields for password login
    let username = request.user.ok_or(StatusCode::BAD_REQUEST)?;
    let password_value = request.password.ok_or(StatusCode::BAD_REQUEST)?;

    // Check if CAPTCHA is required for this login attempt
    let captcha_repo = CaptchaRepository::new(state.db.clone());
    let captcha_service =
        CaptchaService::new(captcha_repo, crate::auth::captcha::CaptchaConfig::from_env());

    let client_ip = addr.ip().to_string();
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");

    // Check if CAPTCHA is required based on rate limiting and suspicious activity
    if captcha_service
        .is_captcha_required(&client_ip, "login")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        // Return CAPTCHA challenge - client must retry with CAPTCHA response
        let _challenge = captcha_service
            .create_challenge(Some(client_ip.clone()), Some(user_agent.to_string()), None)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        info!("CAPTCHA challenge required for login attempt: user={}, ip={}", username, client_ip);

        // Return Matrix M_CAPTCHA_NEEDED error with challenge data
        return Err(StatusCode::TOO_MANY_REQUESTS); // Client should handle this as CAPTCHA needed
    }

    // Create password login request
    let password_request = password::PasswordLoginRequest {
        login_type: request.login_type,
        user: Some(username.clone()),
        identifier: None, // Legacy path uses user field
        password: password_value,
        device_id: request.device_id,
        initial_device_display_name: request.initial_device_display_name,
    };

    // Delegate to password authentication module
    let result = password::post_password_login(
        axum::extract::State(state.clone()),
        ConnectInfo(addr),
        headers,
        cookies.clone(), // Pass cookies to password module
        Json(password_request),
    )
    .await;

    // Record rate limit violations if login failed
    if result.is_err()
        && let Err(e) = captcha_service.record_rate_limit_violation(&username, &client_ip).await
    {
        warn!("Failed to record rate limit violation: {:?}", e);
    }

    // Periodic cleanup of expired CAPTCHA challenges
    tokio::spawn(async move {
        let captcha_repo = CaptchaRepository::new(state.db.clone());
        let captcha_service =
            CaptchaService::new(captcha_repo, crate::auth::captcha::CaptchaConfig::from_env());
        if let Err(e) = captcha_service.cleanup_expired_challenges().await {
            warn!("Failed to cleanup expired CAPTCHA challenges: {:?}", e);
        }
    });

    result
}

async fn handle_token_login(
    state: AppState,
    addr: SocketAddr,
    headers: HeaderMap,
    request: LoginRequest,
) -> Result<Json<LoginResponse>, StatusCode> {
    // Handle SSO and application service authentication
    match request.login_type.as_str() {
        "m.login.sso" => handle_sso_login(state, addr, headers, &request).await,
        "m.login.application_service" => {
            handle_application_service_login(state, addr, headers, &request).await
        },
        _ => {
            warn!("Unsupported login type: {}", request.login_type);
            Err(StatusCode::BAD_REQUEST)
        },
    }
}

async fn handle_refresh_token_login(
    state: AppState,
    refresh_token: String,
    device_id: Option<String>,
) -> Result<Json<LoginResponse>, StatusCode> {
    info!("Processing refresh token login attempt");

    // Validate refresh token and extract claims
    let claims = state.session_service.validate_token(&refresh_token).map_err(|e| {
        warn!("Invalid refresh token: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    // Extract user and device information from claims
    let user_id = claims.matrix_user_id.ok_or_else(|| {
        error!("Refresh token missing user ID");
        StatusCode::UNAUTHORIZED
    })?;

    let current_device_id = claims.matrix_device_id.ok_or_else(|| {
        error!("Refresh token missing device ID");
        StatusCode::UNAUTHORIZED
    })?;

    // Use existing device ID or provided device ID
    let final_device_id = device_id.unwrap_or(current_device_id);

    // Generate new token pair using session service
    let (new_access_token, new_refresh_token) =
        state.session_service.refresh_token(&refresh_token).await.map_err(|e| {
            error!("Failed to refresh tokens: {}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Create structured token pair for organized token management
    let token_pair = TokenPair {
        access_token: new_access_token.clone(),
        refresh_token: new_refresh_token.clone(),
        expires_in: 3600,
        device_id: final_device_id.clone(),
    };

    // Create JWT for the new access token
    let jwt_token = state
        .session_service
        .create_user_token(
            &user_id,
            &final_device_id,
            &token_pair.access_token,
            Some(&token_pair.refresh_token),
            token_pair.expires_in,
        )
        .map_err(|e| {
            error!("Failed to create JWT token: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Refresh token login successful for user: {}", user_id);

    Ok(Json(LoginResponse {
        user_id,
        access_token: jwt_token,
        device_id: token_pair.device_id,
        refresh_token: Some(token_pair.refresh_token),
        expires_in_ms: Some((token_pair.expires_in * 1000) as u64), // Convert to milliseconds
        well_known: None, // Not typically needed for refresh token responses
    }))
}

async fn handle_sso_token_login(
    state: AppState,
    sso_token: String,
    device_id: Option<String>,
) -> Result<Json<LoginResponse>, StatusCode> {
    info!("Processing SSO token login attempt");

    // Validate SSO token using session service
    let claims = state.session_service.validate_token(&sso_token).map_err(|e| {
        warn!("Invalid SSO token: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    // Extract user information from SSO token claims
    let user_id = claims.matrix_user_id.ok_or_else(|| {
        error!("SSO token missing user ID");
        StatusCode::UNAUTHORIZED
    })?;

    // Generate device ID if not provided
    let final_device_id = device_id.unwrap_or_else(|| {
        use uuid::Uuid;
        Uuid::new_v4().simple().to_string().to_uppercase()
    });

    // Create user session using SSO token validation
    let _matrix_access_token = state
        .session_service
        .create_user_session(&user_id, &final_device_id, &sso_token, None)
        .await
        .map_err(|e| {
            error!("Failed to create user session for SSO: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Generate new access token for Matrix API usage
    let access_token = state
        .session_service
        .create_access_token(&user_id, &final_device_id)
        .await
        .map_err(|e| {
            error!("Failed to create access token for SSO user: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Build well-known configuration for SSO response
    let well_known = serde_json::json!({
        "m.homeserver": {
            "base_url": format!("https://{}", state.homeserver_name)
        }
    });

    info!("SSO token login successful for user: {}", user_id);

    Ok(Json(LoginResponse {
        user_id,
        access_token,
        device_id: final_device_id,
        refresh_token: None, // SSO tokens typically don't provide refresh tokens
        expires_in_ms: None, // Token expiration handled by SSO provider
        well_known: Some(well_known),
    }))
}

/// Get configured SSO identity providers from server state using repository
async fn get_configured_sso_providers(state: &AppState) -> Vec<Value> {
    let auth_repo = Arc::new(AuthRepository::new(state.db.clone()));

    match auth_repo.get_sso_providers().await {
        Ok(providers) => providers
            .into_iter()
            .map(|provider| {
                serde_json::json!({
                    "id": provider.id,
                    "name": provider.name,
                    "icon": provider.icon_url,
                    "brand": provider.brand
                })
            })
            .collect(),
        Err(e) => {
            warn!("Failed to query SSO providers: {}", e);
            Vec::new()
        },
    }
}

/// Handle SSO-based login using pre-validated tokens
async fn handle_sso_login(
    state: AppState,
    addr: SocketAddr,
    headers: HeaderMap,
    request: &LoginRequest,
) -> Result<axum::Json<LoginResponse>, axum::http::StatusCode> {
    use axum::Json;
    use uuid::Uuid;

    // SSO login requires a token parameter
    let sso_token = request.token.as_ref().ok_or_else(|| {
        warn!("SSO login missing required 'token' parameter");
        axum::http::StatusCode::BAD_REQUEST
    })?;

    // Validate SSO token and extract user information
    let user_info = validate_sso_token(&state, sso_token).await.map_err(|e| {
        warn!("SSO token validation failed: {}", e);
        axum::http::StatusCode::UNAUTHORIZED
    })?;

    // Generate new device ID if not provided
    let device_id = request
        .device_id
        .clone()
        .unwrap_or_else(|| format!("SSO_{}", Uuid::new_v4().to_string().replace('-', "")));

    // Extract client metadata for device and session tracking
    let client_ip = addr.ip().to_string();
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());

    // Create JWT token for API authentication
    let access_token = state
        .session_service
        .create_access_token(&user_info.user_id, &device_id)
        .await
        .map_err(|e| {
            error!("Failed to create JWT token for SSO user {}: {}", user_info.user_id, e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Store device information using repository
    let device_repo = DeviceRepository::new(state.db.clone());
    device_repo
        .create_device_info(
            &user_info.user_id,
            &device_id,
            request
                .initial_device_display_name
                .clone()
                .or_else(|| Some("SSO Login".to_string())),
            &client_ip,
            user_agent,
            None,
        )
        .await
        .map_err(|e| {
            error!("Failed to store device info for SSO login: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("SSO login successful for user: {}", user_info.user_id);

    Ok(Json(LoginResponse {
        user_id: user_info.user_id,
        access_token,
        device_id,
        refresh_token: None, // SSO typically doesn't use refresh tokens
        expires_in_ms: None, // Could be set based on SSO provider settings
        well_known: None,
    }))
}

/// Handle application service login for bridges and bots
async fn handle_application_service_login(
    state: AppState,
    addr: SocketAddr,
    headers: HeaderMap,
    request: &LoginRequest,
) -> Result<axum::Json<LoginResponse>, axum::http::StatusCode> {
    use axum::Json;
    use uuid::Uuid;

    // Application service login requires a token (AS token)
    let as_token = request.token.as_ref().ok_or_else(|| {
        warn!("Application service login missing required 'token' parameter");
        axum::http::StatusCode::BAD_REQUEST
    })?;

    // Application service login also requires a user parameter (user to login as)
    let target_user = request.user.as_ref().ok_or_else(|| {
        warn!("Application service login missing required 'user' parameter");
        axum::http::StatusCode::BAD_REQUEST
    })?;

    // Validate application service token and permissions
    let app_service = validate_application_service_token(&state, as_token).await.map_err(|e| {
        warn!("Application service token validation failed: {}", e);
        axum::http::StatusCode::UNAUTHORIZED
    })?;

    // Verify the application service can login as this user
    if !can_app_service_login_as(&app_service, target_user) {
        warn!("Application service {} cannot login as user {}", app_service.id, target_user);
        return Err(axum::http::StatusCode::FORBIDDEN);
    }

    // Generate device ID if not provided
    let device_id = request
        .device_id
        .clone()
        .unwrap_or_else(|| format!("AS_{}", Uuid::new_v4().to_string().replace('-', "")));

    // Extract client metadata for device and session tracking
    let client_ip = addr.ip().to_string();
    let user_agent = headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_owned());

    // Create JWT token for API authentication
    let access_token = state
        .session_service
        .create_access_token(target_user, &device_id)
        .await
        .map_err(|e| {
            error!("Failed to create JWT token for AS user {}: {}", target_user, e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Store device information using repository
    let device_repo = DeviceRepository::new(state.db.clone());
    device_repo
        .create_device_info(
            target_user,
            &device_id,
            request
                .initial_device_display_name
                .clone()
                .or_else(|| Some(format!("Application Service: {}", app_service.id))),
            &client_ip,
            user_agent,
            Some(app_service.id.clone()),
        )
        .await
        .map_err(|e| {
            error!("Failed to store device info for AS login: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Application service login successful: {} as user {}", app_service.id, target_user);

    Ok(Json(LoginResponse {
        user_id: target_user.clone(),
        access_token,
        device_id,
        refresh_token: None, // Application services typically don't use refresh tokens
        expires_in_ms: None,
        well_known: None,
    }))
}

/// Validate SSO token and extract user information using repository
async fn validate_sso_token(
    state: &AppState,
    token: &str,
) -> Result<SsoUserInfo, Box<dyn std::error::Error + Send + Sync>> {
    let auth_repo = Arc::new(AuthRepository::new(state.db.clone()));

    let user_info = auth_repo
        .validate_sso_token(token)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
        .ok_or("Invalid or expired SSO token")?;

    Ok(user_info)
}

/// Validate application service token and return service info using repository
async fn validate_application_service_token(
    state: &AppState,
    token: &str,
) -> Result<ApplicationService, Box<dyn std::error::Error + Send + Sync>> {
    let auth_repo = Arc::new(AuthRepository::new(state.db.clone()));

    let service = auth_repo
        .validate_application_service_token(token)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
        .ok_or("Invalid application service token")?;

    Ok(service)
}

/// Check if application service can login as the specified user
fn can_app_service_login_as(app_service: &ApplicationService, user_id: &str) -> bool {
    // Extract localpart from user ID (@localpart:domain)
    let localpart = if let Some(at_pos) = user_id.find('@') {
        if let Some(colon_pos) = user_id.find(':') {
            &user_id[at_pos + 1..colon_pos]
        } else {
            return false;
        }
    } else {
        return false;
    };

    // Check if any user namespace regex matches
    for namespace in &app_service.namespaces.users {
        if let Ok(regex) = regex::Regex::new(&namespace.regex)
            && regex.is_match(localpart)
        {
            return true;
        }
    }

    false
}

// Re-export password module types for convenience
