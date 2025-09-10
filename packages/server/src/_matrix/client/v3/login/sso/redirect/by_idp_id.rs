use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// GET /_matrix/client/v3/login/sso/redirect/{idpId}
pub async fn get(Path(_idp_id): Path<String>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "redirect_url": format!("https://sso.example.com/login/{}", _idp_id)
    })))
}
