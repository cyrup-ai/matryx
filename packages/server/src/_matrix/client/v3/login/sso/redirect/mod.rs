use axum::{Json, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/login/sso/redirect
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "redirect_url": "https://sso.example.com/login"
    })))
}

pub mod by_idp_id;
