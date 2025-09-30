use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::Value;

use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use matryx_surrealdb::repository::profile::ProfileRepository;

/// GET /_matrix/client/v3/profile/{userId}
pub async fn get(
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Path(user_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    // Access authentication fields to ensure they're used
    let _auth_user_id = &auth.user_id;
    let _auth_device_id = auth.get_device_id();

    // Validate user ID format and extract localpart for validation
    let _localpart = if user_id.starts_with('@') {
        if let Some(localpart_end) = user_id.find(':') {
            &user_id[1..localpart_end]
        } else {
            return Err(StatusCode::BAD_REQUEST);
        }
    } else {
        // Use authenticated user's localpart method for comparison
        auth.localpart().ok_or(StatusCode::BAD_REQUEST)?
    };

    // Get user profile from database
    let profile_repo = ProfileRepository::new(state.db.clone());
    let profile_data = profile_repo
        .get_user_profile(&user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Convert profile to JSON response
    let response = serde_json::json!({
        "user_id": profile_data.user_id,
        "display_name": profile_data.display_name,
        "avatar_url": profile_data.avatar_url
    });

    Ok(Json(response))
}

pub mod avatar_url;
pub mod by_key_name;
pub mod displayname;
pub mod report;
