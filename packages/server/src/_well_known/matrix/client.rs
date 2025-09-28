//! GET /.well-known/matrix/client
//! Type: client
//!
//! Returns homeserver discovery information for Matrix clients.
//! Includes auto-discovery validation to ensure the homeserver is reachable.

use crate::config::ServerConfig;
use axum::{http::StatusCode, response::Json};
use reqwest::Client;
use serde_json::{Value, json};
use std::env;
use std::time::Duration;
use tracing::{error, info, warn};

/// Matrix client auto-discovery information with validation
pub async fn get() -> Result<Json<Value>, StatusCode> {
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
    let base_url = format!("https://{}", homeserver_name);

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
        && !identity_server.is_empty() {
        discovery_info.as_object_mut().and_then(|obj| {
            obj.insert(
                "m.identity_server".to_string(),
                json!({
                    "base_url": identity_server
                }),
            )
        });
    }

    Ok(Json(discovery_info))
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
