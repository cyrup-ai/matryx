use axum::{Json, http::StatusCode, extract::State};
use serde_json::{Value, json};
use crate::config::ServerConfig;
use crate::state::AppState;
use crate::auth::captcha::CaptchaService;
use matryx_surrealdb::repository::captcha::CaptchaRepository;

/// GET /_matrix/client/v1/register
pub async fn get(State(state): State<AppState>) -> Result<Json<Value>, StatusCode> {
    // Initialize CAPTCHA service to check if CAPTCHA is enabled
    let captcha_repo = CaptchaRepository::new(state.db.clone());
    let captcha_service = CaptchaService::new(captcha_repo, &state.config.captcha);
    
    // Build registration flows based on server configuration
    let mut flows = vec![
        json!({
            "stages": ["m.login.dummy"]
        })
    ];
    
    // Add CAPTCHA flow if enabled
    if state.config.captcha.enabled {
        flows.push(json!({
            "stages": ["m.login.recaptcha", "m.login.dummy"]
        }));
    }
    
    Ok(Json(json!({
        "flows": flows
    })))
}

/// POST /_matrix/client/v1/register
pub async fn post(
    State(state): State<AppState>,
    Json(payload): Json<Value>
) -> Result<Json<Value>, StatusCode> {
    // Initialize CAPTCHA service
    let captcha_repo = CaptchaRepository::new(state.db.clone());
    let captcha_service = CaptchaService::new(captcha_repo, &state.config.captcha);
    
    // Check if CAPTCHA validation is required
    if state.config.captcha.enabled {
        // Extract CAPTCHA response from request
        if let Some(captcha_response) = payload.get("captcha_response").and_then(|v| v.as_str()) {
            // Validate CAPTCHA response
            let validation_result = captcha_service.validate_captcha_response(captcha_response, "127.0.0.1").await;
            
            match validation_result {
                Ok(true) => {
                    tracing::info!("CAPTCHA validation successful for registration");
                },
                Ok(false) => {
                    tracing::warn!("CAPTCHA validation failed for registration");
                    return Err(StatusCode::BAD_REQUEST);
                },
                Err(e) => {
                    tracing::error!("CAPTCHA validation error: {:?}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        } else {
            // CAPTCHA required but not provided
            tracing::warn!("CAPTCHA required for registration but not provided");
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    
    let config = ServerConfig::get().map_err(|e| {
        tracing::error!("Failed to get server config: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    
    // Clean up expired CAPTCHA challenges periodically
    if let Err(e) = captcha_service.cleanup_expired_challenges().await {
        tracing::warn!("Failed to cleanup expired CAPTCHA challenges: {:?}", e);
    }
    
    Ok(Json(json!({
        "access_token": "syt_example_token",
        "device_id": "EXAMPLE",
        "home_server": config.homeserver_name,
        "user_id": format!("@example:{}", config.homeserver_name)
    })))
}
