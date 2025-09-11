use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_entity::types::{Membership, MembershipState};

#[derive(Deserialize)]
pub struct ForgetRequest {
    // The forget request has no body parameters per Matrix specification
}

#[derive(Serialize)]
pub struct ForgetResponse {
    // Empty response body per Matrix specification
}

/// Matrix Client-Server API v1.11 Section 10.2.8
///
/// POST /_matrix/client/v3/rooms/{roomId}/forget
///
/// Forget a room that the user has previously left. This removes the room from
/// the user's room list and clears their local state for the room. The user
/// must have previously left the room (have "leave" membership state) to be
/// able to forget it.
///
/// This is primarily a client-side operation that removes the user's membership
/// record but does not create any events in the room itself.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(_request): Json<ForgetRequest>,
) -> Result<Json<ForgetResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Room forget failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room forget failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room forget failed - server authentication not allowed for room forget");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room forget failed - anonymous authentication not allowed for room forget");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room forget request for user: {} to forget room: {} (from: {})",
        user_id, room_id, addr
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room forget failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check current membership status
    let current_membership = get_user_membership(&state, &room_id, &user_id).await.map_err(|_| {
        warn!("Room forget failed - user {} has no membership in room {}", user_id, room_id);
        StatusCode::BAD_REQUEST
    })?;

    // User must have left the room to forget it
    match current_membership.membership {
        MembershipState::Leave => {
            // User has left - can forget
        },
        MembershipState::Join => {
            warn!("Room forget failed - user {} is still joined to room {}", user_id, room_id);
            return Err(StatusCode::BAD_REQUEST);
        },
        MembershipState::Invite => {
            warn!("Room forget failed - user {} is invited to room {} (should reject first)", user_id, room_id);
            return Err(StatusCode::BAD_REQUEST);
        },
        MembershipState::Ban => {
            // Users can forget rooms they're banned from
            info!("User {} forgetting banned room {}", user_id, room_id);
        },
        MembershipState::Knock => {
            warn!("Room forget failed - user {} is knocking on room {} (should withdraw first)", user_id, room_id);
            return Err(StatusCode::BAD_REQUEST);
        },
    }

    // Remove the membership record to "forget" the room
    let membership_id = format!("{}:{}", user_id, room_id);
    let delete_result: Result<Option<Membership>, _> = state.db.delete(("membership", &membership_id)).await;

    match delete_result {
        Ok(deleted_membership) => {
            if deleted_membership.is_some() {
                info!("Successfully forgot room {} for user {}", room_id, user_id);
            } else {
                // Membership was already deleted or didn't exist - idempotent operation
                info!("Room {} was already forgotten for user {}", room_id, user_id);
            }
        },
        Err(e) => {
            error!("Failed to delete membership record for user {} in room {}: {}", user_id, room_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    }

    // Optionally clean up user-specific room state (account data, etc.)
    if let Err(e) = cleanup_user_room_state(&state, &room_id, &user_id).await {
        warn!("Failed to clean up room state for user {} in room {}: {}", user_id, room_id, e);
        // Don't fail the request - this is cleanup only
    }

    Ok(Json(ForgetResponse {}))
}

async fn get_user_membership(
    state: &AppState,
    room_id: &str,
    user_id: &str,
) -> Result<Membership, Box<dyn std::error::Error + Send + Sync>> {
    let membership_id = format!("{}:{}", user_id, room_id);
    let membership: Option<Membership> = state.db.select(("membership", membership_id)).await?;

    membership.ok_or_else(|| "Membership not found".into())
}

/// Clean up user-specific room state when forgetting a room
///
/// This removes account data, tags, and other user-specific data associated
/// with the room. This cleanup is optional and failures don't prevent the
/// forget operation from succeeding.
async fn cleanup_user_room_state(
    state: &AppState,
    room_id: &str,
    user_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Clean up room-specific account data
    let account_data_query = "
        DELETE FROM account_data 
        WHERE user_id = $user_id 
          AND scope = 'room' 
          AND scope_id = $room_id
    ";

    if let Err(e) = state
        .db
        .query(account_data_query)
        .bind(("user_id", user_id.to_string()))
        .bind(("room_id", room_id.to_string()))
        .await
    {
        warn!("Failed to clean up account data for user {} in room {}: {}", user_id, room_id, e);
    }

    // Clean up room tags
    let tags_query = "
        DELETE FROM room_tags 
        WHERE user_id = $user_id 
          AND room_id = $room_id
    ";

    if let Err(e) = state
        .db
        .query(tags_query)
        .bind(("user_id", user_id.to_string()))
        .bind(("room_id", room_id.to_string()))
        .await
    {
        warn!("Failed to clean up room tags for user {} in room {}: {}", user_id, room_id, e);
    }

    // Clean up read receipts for this user in this room
    let receipts_query = "
        DELETE FROM room_receipts 
        WHERE user_id = $user_id 
          AND room_id = $room_id
    ";

    if let Err(e) = state
        .db
        .query(receipts_query)
        .bind(("user_id", user_id.to_string()))
        .bind(("room_id", room_id.to_string()))
        .await
    {
        warn!("Failed to clean up receipts for user {} in room {}: {}", user_id, room_id, e);
    }

    // Clean up typing indicators for this user in this room
    let typing_query = "
        DELETE FROM room_typing 
        WHERE user_id = $user_id 
          AND room_id = $room_id
    ";

    if let Err(e) = state
        .db
        .query(typing_query)
        .bind(("user_id", user_id.to_string()))
        .bind(("room_id", room_id.to_string()))
        .await
    {
        warn!("Failed to clean up typing indicators for user {} in room {}: {}", user_id, room_id, e);
    }

    info!("Cleaned up room state for user {} in forgotten room {}", user_id, room_id);
    Ok(())
}
