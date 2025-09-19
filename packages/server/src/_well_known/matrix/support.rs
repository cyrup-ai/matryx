//! GET /.well-known/matrix/support
//! Type: client
//!
//! Returns information about how to contact the server administrator for support.
//! This is an optional endpoint that helps users find support when things go wrong.

use crate::config::ServerConfig;
use axum::{http::StatusCode, response::Json};
use serde_json::{Value, json};
use std::env;

/// Matrix server support information
pub async fn get() -> Result<Json<Value>, StatusCode> {
    // Get configuration from centralized ServerConfig
    let config = ServerConfig::get();
    let homeserver_name = &config.homeserver_name;
    let admin_email = &config.admin_email;

    let support_page = env::var("MATRIX_SUPPORT_PAGE")
        .unwrap_or_else(|_| format!("https://{}/support", homeserver_name));

    // Return standard Matrix support information
    // This provides contact information for users who need help
    let support_info = json!({
        "admins": [
            {
                "matrix_id": format!("@admin:{}", homeserver_name),
                "email_address": admin_email,
                "role": "administrator"
            }
        ],
        "support_page": support_page
    });

    Ok(Json(support_info))
}
