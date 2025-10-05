//! GET /.well-known/matrix/identity_server  
//! Type: client
//!
//! Returns identity server discovery information for Matrix clients.
//! Enables identity server integration for the Matrix ecosystem.

use axum::http::StatusCode;
use reqwest::Client;
use serde_json::{Value, json};
use std::env;
use std::time::Duration;
use tracing::{error, info, warn};

/// Matrix identity server discovery information with validation
pub async fn get() -> Result<impl axum::response::IntoResponse, StatusCode> {
    // Get identity server URL from environment configuration
    let identity_server_url = match env::var("MATRIX_IDENTITY_SERVER") {
        Ok(url) => {
            if url.is_empty() {
                error!("MATRIX_IDENTITY_SERVER environment variable is empty");
                return Err(StatusCode::NOT_FOUND);
            }
            url
        },
        Err(_) => {
            // Identity server is optional - return 404 if not configured
            info!("No identity server configured (MATRIX_IDENTITY_SERVER not set)");
            return Err(StatusCode::NOT_FOUND);
        },
    };

    // Validate the identity server URL format
    if !identity_server_url.starts_with("https://") && !identity_server_url.starts_with("http://") {
        error!("Invalid identity server URL format: {}", identity_server_url);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Validate the identity server by checking /_matrix/identity/v2 endpoint
    // This implements Matrix specification requirement for identity server validation
    match validate_identity_server_reachability(&identity_server_url).await {
        Ok(true) => {
            info!("Identity server validation successful for: {}", identity_server_url);
        },
        Ok(false) => {
            warn!("Identity server validation failed for: {}", identity_server_url);
            // Still serve the endpoint but log the issue
        },
        Err(e) => {
            error!("Error during identity server validation: {}", e);
            // Still serve the endpoint but log the error
        },
    }

    // Return identity server discovery information
    let discovery_info = json!({
        "m.identity_server": {
            "base_url": identity_server_url
        }
    });

    Ok(axum::response::Json(discovery_info))
}

/// Validates that an identity server is reachable by checking /_matrix/identity/v2 endpoint
/// Returns Ok(true) if validation passes, Ok(false) if identity server is unreachable,
/// Err(e) if there was an error during validation
async fn validate_identity_server_reachability(base_url: &str) -> Result<bool, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let identity_url = format!("{}/_matrix/identity/v2", base_url);

    match client.get(&identity_url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                // Try to parse the response - should return empty object {}
                match response.json::<Value>().await {
                    Ok(json_data) => {
                        // According to Matrix spec, should return empty object
                        if json_data.is_object() {
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
