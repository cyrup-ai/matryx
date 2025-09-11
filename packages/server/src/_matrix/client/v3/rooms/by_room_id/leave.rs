use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    _matrix::client::v3::create_room::get_user_avatar_url,
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
    utils::matrix_events::{calculate_content_hashes, sign_event},
};
use matryx_entity::types::{Event, EventContent, Membership, MembershipState, Room};

#[derive(Deserialize)]
pub struct LeaveRequest {
    /// Optional reason for leaving the room
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct LeaveResponse {
    // Empty response body per Matrix specification
}

/// Matrix Client-Server API v1.11 Section 10.2.3
///
/// POST /_matrix/client/v3/rooms/{roomId}/leave
///
/// Leave a room that the user is currently joined to. This endpoint creates
/// a leave membership event for the authenticated user in the specified room.
///
/// The user must currently be a member of the room (have "join" membership)
/// to be able to leave it. After leaving, the user will no longer receive
/// events from the room unless they rejoin.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<LeaveRequest>,
) -> Result<Json<LeaveResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Room leave failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room leave failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room leave failed - server authentication not allowed for room leaves");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room leave failed - anonymous authentication not allowed for room leaves");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room leave request for user: {} from room: {} (from: {})",
        user_id, room_id, addr
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room leave failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if room exists
    let room: Option<Room> = state.db.select(("room", &room_id)).await.map_err(|e| {
        error!("Failed to query room {}: {}", room_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if room.is_none() {
        warn!("Room leave failed - room not found: {}", room_id);
        return Err(StatusCode::NOT_FOUND);
    }

    // Check current membership status
    let current_membership =
        get_user_membership(&state, &room_id, &user_id).await.map_err(|_| {
            warn!(
                "Room leave failed - could not determine membership for user {} in room {}",
                user_id, room_id
            );
            StatusCode::FORBIDDEN
        })?;

    // Validate that user is currently joined
    match current_membership.membership {
        MembershipState::Join => {
            // User is joined - can leave
        },
        MembershipState::Invite => {
            // User is invited - can reject invitation (which is effectively leaving)
            info!("User {} is rejecting invitation to room {}", user_id, room_id);
        },
        MembershipState::Leave => {
            // User already left - this is idempotent, return success
            info!("User {} already left room {}", user_id, room_id);
            return Ok(Json(LeaveResponse {}));
        },
        MembershipState::Ban => {
            warn!("Room leave failed - user {} is banned from room {}", user_id, room_id);
            return Err(StatusCode::FORBIDDEN);
        },
        MembershipState::Knock => {
            // User has knocked - can withdraw knock (effectively leaving)
            info!("User {} is withdrawing knock from room {}", user_id, room_id);
        },
    }

    // Get event context for the leave event
    let event_depth = get_next_event_depth(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let prev_events = get_latest_event_ids(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_events = get_auth_events_for_leave(&state, &room_id, &user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create leave membership event
    let leave_event_id = create_membership_event(
        &state,
        &room_id,
        &user_id,
        &user_id,
        MembershipState::Leave,
        request.reason.as_deref(),
        event_depth,
        &prev_events,
        &auth_events,
    )
    .await
    .map_err(|e| {
        error!("Failed to create leave event for user {} in room {}: {}", user_id, room_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Successfully created leave event {} for user {} in room {}",
        leave_event_id, user_id, room_id
    );

    Ok(Json(LeaveResponse {}))
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

async fn get_next_event_depth(
    state: &AppState,
    room_id: &str,
) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    // Query the maximum depth in the room and add 1
    let query = "SELECT VALUE math::max(depth) FROM event WHERE room_id = $room_id";
    let mut result = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    let max_depth: Option<i64> = result.take(0)?;
    Ok(max_depth.unwrap_or(0) + 1)
}

async fn get_latest_event_ids(
    state: &AppState,
    room_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get the most recent events to use as prev_events
    // This is a simplified approach - in a full implementation, this would
    // need to consider the event DAG structure more carefully
    let query = "SELECT event_id FROM event WHERE room_id = $room_id ORDER BY depth DESC LIMIT 3";
    let mut result = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    let events: Vec<String> = result.take(0)?;
    Ok(events)
}

async fn get_auth_events_for_leave(
    state: &AppState,
    room_id: &str,
    user_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get the auth events needed for a leave event:
    // - m.room.create event
    // - m.room.power_levels event
    // - Current user's m.room.member event (join state)

    let query = r#"
        SELECT event_id FROM event 
        WHERE room_id = $room_id 
        AND ((event_type = 'm.room.create' AND state_key = '')
             OR (event_type = 'm.room.power_levels' AND state_key = '')
             OR (event_type = 'm.room.member' AND state_key = $user_id))
        ORDER BY origin_server_ts ASC
    "#;

    let mut result = state
        .db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .bind(("user_id", user_id.to_string()))
        .await?;

    let auth_events: Vec<String> = result.take(0)?;
    Ok(auth_events)
}

async fn create_membership_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    target: &str,
    membership: MembershipState,
    reason: Option<&str>,
    depth: i64,
    prev_events: &[String],
    auth_events: &[String],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let event_id = format!("${}:{}", Uuid::new_v4(), state.homeserver_name);

    let mut content = json!({
        "membership": match membership {
            MembershipState::Join => "join",
            MembershipState::Leave => "leave",
            MembershipState::Invite => "invite",
            MembershipState::Ban => "ban",
            MembershipState::Knock => "knock",
        }
    });

    if let Some(reason) = reason {
        content["reason"] = json!(reason);
    }

    // Create the event with proper hashes and signatures
    let mut event = Event {
        event_id: event_id.clone(),
        room_id: room_id.to_string(),
        sender: sender.to_string(),
        event_type: "m.room.member".to_string(),
        content: EventContent::Unknown(content.clone()),
        state_key: Some(target.to_string()),
        origin_server_ts: Utc::now().timestamp_millis(),
        unsigned: None,
        prev_events: Some(prev_events.to_vec()),
        auth_events: Some(auth_events.to_vec()),
        depth: Some(depth),
        hashes: serde_json::from_value(json!({})).ok(),
        signatures: serde_json::from_value(json!({})).ok(),
        redacts: None,
        outlier: Some(false),
        rejected_reason: None,
        soft_failed: Some(false),
        received_ts: Some(Utc::now().timestamp_millis()),
    };

    // Calculate content hashes according to Matrix specification
    let hashes_value = crate::utils::matrix_events::calculate_content_hashes(&event)?;
    let hashes: HashMap<String, String> = serde_json::from_value(hashes_value)?;
    event.hashes = Some(hashes);

    // Sign event with server's Ed25519 private key
    let signatures_value = crate::utils::matrix_events::sign_event(state, &event).await?;
    let signatures: HashMap<String, HashMap<String, String>> =
        serde_json::from_value(signatures_value)?;
    event.signatures = Some(signatures);

    // Store the event
    let _: Option<Event> = state.db.create(("event", &event_id)).content(event).await?;

    // Get existing membership to preserve display name
    let membership_id = format!("{}:{}", target, room_id);
    let existing_membership: Option<Membership> =
        match state.db.select(("membership", &membership_id)).await {
            Ok(membership) => membership,
            Err(e) => {
                error!("Failed to get existing membership for {}: {}", membership_id, e);
                None // Just use None if we can't get existing membership
            },
        };

    // Preserve existing display name when leaving
    let preserved_display_name = existing_membership.as_ref().and_then(|m| m.display_name.clone());

    // Update membership record
    let membership_record = Membership {
        user_id: target.to_string(),
        room_id: room_id.to_string(),
        membership: membership.clone(),
        reason: reason.map(|r| r.to_string()),
        invited_by: None, // Not applicable for leave events
        updated_at: Some(Utc::now()),
        display_name: preserved_display_name,
        avatar_url: get_user_avatar_url(&state, target)
            .await
            .map_err(|e| {
                error!("Failed to get avatar URL for user {}: {}", target, e);
                e
            })
            .ok()
            .flatten(),
        is_direct: Some(is_direct_message_room(&state, room_id).await.unwrap_or(false)),
        third_party_invite: None,
        join_authorised_via_users_server: None,
    };

    let _: Option<Membership> = state
        .db
        .upsert(("membership", membership_id))
        .content(membership_record)
        .await?;

    Ok(event_id)
}

// Removed duplicate function - using shared implementation from create_room.rs

/// Determine if a room is a direct message room
async fn is_direct_message_room(
    state: &AppState,
    room_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let query = "SELECT is_direct FROM rooms WHERE room_id = $room_id";
    let room_id_owned = room_id.to_string();

    let mut response = state.db.query(query).bind(("room_id", room_id_owned)).await?;

    #[derive(serde::Deserialize)]
    struct RoomInfo {
        is_direct: bool,
    }

    let rooms: Vec<RoomInfo> = response.take(0)?;
    Ok(rooms.into_iter().next().map(|r| r.is_direct).unwrap_or(false))
}
