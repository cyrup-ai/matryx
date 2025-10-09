//! GET /.well-known/matrix/client
//! Type: client
//!
//! Returns homeserver discovery information for Matrix clients.
//! Includes auto-discovery validation to ensure the homeserver is reachable.

use crate::auth::CaptchaService;
use crate::auth::captcha::CaptchaConfig;
use crate::config::ServerConfig;
use axum::http::StatusCode;
use matryx_surrealdb::repository::CaptchaRepository;
use reqwest::Client;
use serde_json::{Value, json};
use std::env;
use std::time::Duration;
use tracing::{error, info, warn};

/// Matrix client auto-discovery information with validation
pub async fn get() -> Result<impl axum::response::IntoResponse, StatusCode> {
    // Get configuration from centralized ServerConfig
    let config = match ServerConfig::get() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to get server configuration for client discovery: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    let homeserver_name = &config.homeserver_name;

    // Validate homeserver name format
    if homeserver_name.is_empty() {
        error!("Invalid homeserver name: empty string");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Construct base URL for the homeserver
    let base_url = config.base_url();

    // Validate the homeserver base URL by checking /_matrix/client/versions endpoint
    // This implements Matrix specification requirement for auto-discovery validation
    match validate_homeserver_reachability(&base_url).await {
        Ok(true) => {
            info!("Homeserver validation successful for: {}", base_url);
        },
        Ok(false) => {
            warn!("Homeserver validation failed for: {}", base_url);
            // Continue serving the endpoint but log the issue
        },
        Err(e) => {
            error!("Error during homeserver validation: {}", e);
            // Continue serving the endpoint but log the error
        },
    }

    // Prepare discovery information
    let mut discovery_info = json!({
        "m.homeserver": {
            "base_url": base_url
        }
    });

    // Add identity server information if configured
    if let Ok(identity_server) = env::var("MATRIX_IDENTITY_SERVER")
        && !identity_server.is_empty()
    {
        discovery_info.as_object_mut().and_then(|obj| {
            obj.insert(
                "m.identity_server".to_string(),
                json!({
                    "base_url": identity_server
                }),
            )
        });
    }

    // Add CAPTCHA configuration for Matrix clients
    // This allows clients to know if CAPTCHA is enabled and what provider is used
    add_captcha_config(&mut discovery_info).await;

    Ok(axum::response::Json(discovery_info))
}

/// Validates that a homeserver is reachable by checking /_matrix/client/versions endpoint
/// Returns Ok(true) if validation passes, Ok(false) if homeserver is unreachable,
/// Err(e) if there was an error during validation
async fn validate_homeserver_reachability(base_url: &str) -> Result<bool, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let versions_url = format!("{}/_matrix/client/versions", base_url);

    match client.get(&versions_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                // Try to parse the response to ensure it's valid Matrix versions endpoint
                match response.json::<Value>().await {
                    Ok(json_data) => {
                        // Check if the response contains the expected "versions" field
                        if json_data.get("versions").is_some() {
                            Ok(true)
                        } else {
                            Ok(false)
                        }
                    },
                    Err(_) => Ok(false),
                }
            } else {
                Ok(false)
            }
        },
        Err(e) => Err(format!("HTTP request failed: {}", e)),
    }
}

/// Add CAPTCHA configuration to the well-known client discovery response
/// This integrates unused CaptchaService methods into the Matrix specification
async fn add_captcha_config(discovery_info: &mut Value) {
    // Get database connection from environment or use default
    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| "memory".to_string());

    if let Ok(db) = surrealdb::engine::any::connect(&db_url).await {
        let captcha_config = CaptchaConfig::from_env();
        let captcha_repo = CaptchaRepository::new(db);
        let captcha_service = CaptchaService::new(captcha_repo, captcha_config);

        // Get CAPTCHA public configuration for clients
        let captcha_client_config = captcha_service.get_public_config();

        if !captcha_client_config.is_empty()
            && let Some(obj) = discovery_info.as_object_mut()
        {
            obj.insert("m.captcha".to_string(), json!(captcha_client_config));
        }

        info!("Added CAPTCHA configuration to client discovery");
    } else {
        warn!("Failed to connect to database for CAPTCHA configuration");
    }
}
