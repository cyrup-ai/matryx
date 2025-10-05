//! GET /.well-known/matrix/support
//! Type: client
//!
//! Returns information about how to contact the server administrator for support.
//! This is an optional endpoint that helps users find support when things go wrong.

use crate::config::{ServerConfig, SupportConfig};
use axum::{http::StatusCode, response::Json};
use serde_json::{Value, json};

use tracing::{error, warn};

/// Matrix server support information
/// Returns JSON object with administrator contact information and support page URL
pub async fn get() -> Result<Json<Value>, StatusCode> {
    // Get configuration from centralized ServerConfig
    let server_config = match ServerConfig::get() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to get server configuration for support endpoint: {:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Create support configuration from environment and server config
    let support_config =
        SupportConfig::from_env(&server_config.homeserver_name, &server_config.admin_email);

    // Validate support configuration
    if let Err(validation_error) = support_config.validate() {
        error!("Support configuration validation failed: {}", validation_error);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Check if public support is enabled
    if !support_config.public_support_enabled {
        warn!("Public support information is disabled");
        return Err(StatusCode::NOT_FOUND);
    }

    // Build contacts array from support configuration
    let mut contacts = Vec::new();
    for contact in support_config.get_support_contacts() {
        contacts.push(json!({
            "matrix_id": contact.matrix_id,
            "email_address": contact.email_address,
            "role": contact.role
        }));
    }

    // Build response object
    let mut support_info = json!({
        "contacts": contacts
    });

    // Add support page if enabled
    if let Some(support_page_url) = support_config.get_support_page_url() {
        support_info
            .as_object_mut()
            .and_then(|obj| obj.insert("support_page".to_string(), json!(support_page_url)));
    }

    Ok(Json(support_info))
}
