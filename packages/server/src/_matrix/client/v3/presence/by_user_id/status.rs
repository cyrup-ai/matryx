use axum::{Json, extract::{Path, State}, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{AppState, auth::{MatrixAuth, extract_matrix_auth}};

#[derive(Debug, Serialize)]
pub struct PresenceResponse {
    /// The user's presence state (one of: online, offline, unavailable)
    pub presence: String,
    /// The length of time in milliseconds since an action was performed by this user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_ago: Option<u64>,
    /// The user's status message if they have set one
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_msg: Option<String>,
    /// Whether the user is currently active
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currently_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PresenceRequest {
    /// The new presence state (one of: online, offline, unavailable)
    pub presence: String,
    /// The status message to attach to this state
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_msg: Option<String>,
}

/// GET /_matrix/client/v3/presence/{userId}/status
///
/// Get the presence state of a user.
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<PresenceResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Presence get failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let _authenticated_user = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Presence get failed - access token expired");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Presence get failed - server authentication not allowed");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Presence get failed - anonymous authentication not allowed");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Getting presence for user: {}", user_id);

    // Get user presence from repository
    match state.presence_repo.get_user_presence(&user_id).await {
        Ok(Some(presence_update)) => {
            // Convert last_active_ago from i64 to u64 if present
            let last_active_ago = presence_update.last_active_ago.and_then(|val| {
                if val >= 0 {
                    Some(val as u64)
                } else {
                    None
                }
            });

            info!("Retrieved presence for user {}: {}", user_id, presence_update.presence);

            Ok(Json(PresenceResponse {
                presence: presence_update.presence,
                last_active_ago,
                status_msg: presence_update.status_msg,
                currently_active: presence_update.currently_active,
            }))
        },
        Ok(None) => {
            // User has no presence set, return offline as default
            info!("No presence found for user {}, returning offline", user_id);
            Ok(Json(PresenceResponse {
                presence: "offline".to_string(),
                last_active_ago: None,
                status_msg: None,
                currently_active: Some(false),
            }))
        },
        Err(e) => {
            error!("Failed to get user presence: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
    }
}

/// PUT /_matrix/client/v3/presence/{userId}/status
///
/// Set the presence state of the user.
pub async fn put(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(payload): Json<PresenceRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Presence update failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let authenticated_user = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Presence update failed - access token expired");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Presence update failed - server authentication not allowed");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Presence update failed - anonymous authentication not allowed");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    // Validate user is updating their own presence
    if authenticated_user != user_id {
        warn!("User {} attempted to update presence for {}", authenticated_user, user_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate presence state
    if payload.presence != "online" && payload.presence != "offline" && payload.presence != "unavailable" {
        warn!("Invalid presence state: {}", payload.presence);
        return Err(StatusCode::BAD_REQUEST);
    }

    info!("Setting presence for user {} to {}", user_id, payload.presence);

    // Update presence based on state
    let result = match payload.presence.as_str() {
        "online" => {
            state.presence_repo.set_user_online(&user_id, payload.status_msg).await
        },
        "offline" => {
            state.presence_repo.set_user_offline(&user_id).await
        },
        "unavailable" => {
            state.presence_repo.set_user_unavailable(&user_id, payload.status_msg).await
        },
        _ => {
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    match result {
        Ok(()) => {
            info!("Successfully updated presence for user {}", user_id);
            Ok(Json(json!({})))
        },
        Err(e) => {
            error!("Failed to update presence: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        },
    }
}
