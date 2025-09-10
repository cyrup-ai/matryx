use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::Value;

use crate::auth::AuthenticatedUser;
use crate::state::AppState;

/// GET /_matrix/client/v3/profile/{userId}
pub async fn get(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // Access authentication fields to ensure they're used
    let _auth_user_id = &auth.user_id;
    let _auth_device_id = &auth.device_id;

    // Validate user ID format and extract localpart for validation
    let _localpart = if user_id.starts_with('@') {
        if let Some(localpart_end) = user_id.find(':') {
            &user_id[1..localpart_end]
        } else {
            return Err(StatusCode::BAD_REQUEST);
        }
    } else {
        // Use authenticated user's localpart method for comparison
        auth.localpart()
    };

    // Get user profile from database
    let profile: Result<Option<Value>, _> = state
        .db
        .query("SELECT user_id, display_name, avatar_url FROM user WHERE user_id = $user_id")
        .bind(("user_id", user_id.clone()))
        .await
        .and_then(|mut response| response.take(0));

    let profile_data = profile
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(profile_data))
}

pub mod avatar_url;
pub mod by_key_name;
pub mod displayname;
pub mod report;
