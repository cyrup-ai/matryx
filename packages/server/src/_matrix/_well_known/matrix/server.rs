use axum::{Json, http::StatusCode};
use serde_json::{Value, json};
use std::env;

/// GET /.well-known/matrix/server
/// 
/// Returns server delegation information for Matrix federation.
/// This tells other homeservers where to connect for federation.
pub async fn get() -> Result<Json<Value>, StatusCode> {
    // Get the server name from environment or use default
    let server_name = env::var("HOMESERVER_NAME")
        .unwrap_or_else(|_| "localhost:8448".to_string());
    
    Ok(Json(json!({
        "m.server": server_name
    })))
}