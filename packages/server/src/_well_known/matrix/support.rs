//! GET /.well-known/matrix/support
//! Type: client
//!
//! Returns information about how to contact the server administrator for support.
//! This is an optional endpoint that helps users find support when things go wrong.

use axum::{http::StatusCode, response::Json};
use serde_json::{Value, json};
use std::env;
use tracing::warn;

/// Matrix server support information
pub async fn get() -> Result<Json<Value>, StatusCode> {
    // Get configuration from environment variables with reasonable defaults
    let homeserver_name = env::var("HOMESERVER_NAME").unwrap_or_else(|_| {
        warn!("HOMESERVER_NAME environment variable not set, using 'localhost'");
        "localhost".to_string()
    });

    let admin_email = env::var("MATRIX_ADMIN_EMAIL").unwrap_or_else(|_| {
        warn!("MATRIX_ADMIN_EMAIL environment variable not set, using default");
        format!("admin@{}", homeserver_name)
    });

    let support_page = env::var("MATRIX_SUPPORT_PAGE").unwrap_or_else(|_| {
        warn!("MATRIX_SUPPORT_PAGE environment variable not set, using default");
        format!("https://{}/support", homeserver_name)
    });

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
