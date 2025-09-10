use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};

use crate::auth::AuthenticatedUser;
use crate::state::AppState;

/// GET /_matrix/client/v3/admin/whois/{userId}
pub async fn get(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // Check if user is admin
    if !auth.is_admin(&state).await {
        return Err(StatusCode::FORBIDDEN);
    }

    // Access authentication fields to ensure they're used
    let _auth_user_id = &auth.user_id;
    let _auth_device_id = &auth.device_id;

    // Get user information from database
    let user_info: Result<Option<Value>, _> = state.db
        .query("SELECT user_id, display_name, avatar_url, is_admin, created_ts, is_active FROM user WHERE user_id = $user_id")
        .bind(("user_id", user_id.clone()))
        .await
        .and_then(|mut response| response.take(0));

    let user_data = user_info
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get user devices
    let devices: Result<Vec<Value>, _> = state.db
        .query("SELECT device_id, display_name, last_seen_ip, last_seen_ts FROM device WHERE user_id = $user_id")
        .bind(("user_id", user_id.clone()))
        .await
        .and_then(|mut response| response.take(0));

    let device_list = devices.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "user_id": user_id,
        "user_info": user_data,
        "devices": device_list
    })))
}
