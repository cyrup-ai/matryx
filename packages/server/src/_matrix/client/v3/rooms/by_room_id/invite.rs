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
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
    utils::matrix_events::{calculate_content_hashes, sign_event},
};
use matryx_entity::types::{Event, EventContent, Membership, MembershipState, Room};

#[derive(Deserialize)]
pub struct InviteRequest {
    /// The Matrix user ID of the user to invite
    pub user_id: String,

    /// Optional reason for the invitation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct InviteResponse {
    // Empty response body per Matrix specification
}

/// Matrix Client-Server API v1.11 Section 10.2.2
///
/// POST /_matrix/client/v3/rooms/{roomId}/invite
///
/// Invite a user to join a room. The authenticated user must have permission
/// to invite users to the room (based on power levels and room configuration).
///
/// The invited user will receive an invitation event and can choose to accept
/// or reject the invitation using the join or leave endpoints respectively.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<InviteRequest>,
) -> Result<Json<InviteResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Room invite failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let inviter_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room invite failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room invite failed - server authentication not allowed for room invites");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room invite failed - anonymous authentication not allowed for room invites");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room invitation from user: {} to invite: {} to room: {} (from: {})",
        inviter_id, request.user_id, room_id, addr
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room invite failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate user ID format
    if !request.user_id.starts_with('@') || !request.user_id.contains(':') {
        warn!("Room invite failed - invalid user ID format: {}", request.user_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if room exists
    let room: Option<Room> = state.db.select(("room", &room_id)).await.map_err(|e| {
        error!("Failed to query room {}: {}", room_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let room = room.ok_or_else(|| {
        warn!("Room invite failed - room not found: {}", room_id);
        StatusCode::NOT_FOUND
    })?;

    // Check if inviter is a member of the room with appropriate permissions
    let inviter_membership =
        get_user_membership(&state, &room_id, &inviter_id).await.map_err(|_| {
            warn!(
                "Room invite failed - inviter {} is not a member of room {}",
                inviter_id, room_id
            );
            StatusCode::FORBIDDEN
        })?;

    if inviter_membership.membership != MembershipState::Join {
        warn!(
            "Room invite failed - inviter {} must be joined to room {} to send invites",
            inviter_id, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Check inviter's power level and permission to invite
    if !can_user_invite(&state, &room, &inviter_id).await? {
        warn!(
            "Room invite failed - user {} does not have permission to invite in room {}",
            inviter_id, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Check current membership status of the invitee
    match get_user_membership(&state, &room_id, &request.user_id).await {
        Ok(existing_membership) => {
            match existing_membership.membership {
                MembershipState::Join => {
                    info!("User {} is already joined to room {}", request.user_id, room_id);
                    return Ok(Json(InviteResponse {}));
                },
                MembershipState::Invite => {
                    // User is already invited - this is idempotent, return success
                    info!("User {} is already invited to room {}", request.user_id, room_id);
                    return Ok(Json(InviteResponse {}));
                },
                MembershipState::Ban => {
                    warn!(
                        "Room invite failed - user {} is banned from room {}",
                        request.user_id, room_id
                    );
                    return Err(StatusCode::FORBIDDEN);
                },
                MembershipState::Leave | MembershipState::Knock => {
                    // User previously left or knocked - can be invited
                },
            }
        },
        Err(_) => {
            // User has no membership - can be invited
        },
    }

    // Get event context for the invite event
    let event_depth = get_next_event_depth(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let prev_events = get_latest_event_ids(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_events = get_auth_events_for_invite(&state, &room_id, &inviter_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create invite membership event
    let invite_event_id = create_membership_event(
        &state,
        &room_id,
        &inviter_id,
        &request.user_id,
        MembershipState::Invite,
        request.reason.as_deref(),
        event_depth,
        &prev_events,
        &auth_events,
    )
    .await
    .map_err(|e| {
        error!(
            "Failed to create invite event from {} to {} in room {}: {}",
            inviter_id, request.user_id, room_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Successfully created invite event {} from {} to {} in room {}",
        invite_event_id, inviter_id, request.user_id, room_id
    );

    Ok(Json(InviteResponse {}))
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

async fn can_user_invite(state: &AppState, room: &Room, user_id: &str) -> Result<bool, StatusCode> {
    // Get the power levels from the room
    let power_levels_value = room
        .power_levels
        .as_ref()
        .map(|pl| serde_json::to_value(pl).unwrap_or_default())
        .unwrap_or_default();
    let power_levels = match power_levels_value.as_object() {
        Some(levels) => levels,
        None => {
            error!("Room {} has invalid power levels format", room.room_id);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Get the user's power level
    let user_power_level =
        if let Some(users) = power_levels.get("users").and_then(|u| u.as_object()) {
            users.get(user_id).and_then(|p| p.as_i64()).unwrap_or(0)
        } else {
            0
        };

    // Get the required power level to invite (default 0 per Matrix spec)
    let required_invite_level = power_levels.get("invite").and_then(|i| i.as_i64()).unwrap_or(0);

    Ok(user_power_level >= required_invite_level)
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
    let query = "SELECT event_id FROM event WHERE room_id = $room_id ORDER BY depth DESC LIMIT 3";
    let mut result = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    let events: Vec<String> = result.take(0)?;
    Ok(events)
}

async fn get_auth_events_for_invite(
    state: &AppState,
    room_id: &str,
    inviter_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get the auth events needed for an invite event:
    // - m.room.create event
    // - m.room.join_rules event
    // - m.room.power_levels event
    // - Inviting user's m.room.member event (join state)

    let query = r#"
        SELECT event_id FROM event 
        WHERE room_id = $room_id 
        AND ((event_type = 'm.room.create' AND state_key = '')
             OR (event_type = 'm.room.join_rules' AND state_key = '')
             OR (event_type = 'm.room.power_levels' AND state_key = '')
             OR (event_type = 'm.room.member' AND state_key = $inviter_id))
        ORDER BY origin_server_ts ASC
    "#;

    let mut result = state
        .db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .bind(("inviter_id", inviter_id.to_string()))
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

    // Create the event
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
    let hashes_value = calculate_content_hashes(&event)?;
    let hashes: HashMap<String, String> = serde_json::from_value(hashes_value)?;
    event.hashes = Some(hashes);

    // Sign event with server's Ed25519 private key
    let signatures_value = sign_event(state, &event).await?;
    let signatures: HashMap<String, HashMap<String, String>> =
        serde_json::from_value(signatures_value)?;
    event.signatures = Some(signatures);

    // Store the event
    let _: Option<Event> = state.db.create(("event", &event_id)).content(event).await?;

    // Create/update membership record
    let membership_record = Membership {
        user_id: target.to_string(),
        room_id: room_id.to_string(),
        membership: membership.clone(),
        reason: reason.map(|r| r.to_string()),
        invited_by: if membership == MembershipState::Invite {
            Some(sender.to_string())
        } else {
            None
        },
        updated_at: Some(Utc::now()),
        display_name: get_user_display_name(&state, target).await,
        avatar_url: get_user_avatar_url(&state, target).await,
        is_direct: Some(is_direct_message_room(&state, room_id).await.unwrap_or(false)),
        third_party_invite: None,
        join_authorised_via_users_server: None,
    };

    let membership_id = format!("{}:{}", target, room_id);
    let _: Option<Membership> = state
        .db
        .upsert(("membership", membership_id))
        .content(membership_record)
        .await?;

    Ok(event_id)
}

/// Get user display name from profile
async fn get_user_display_name(state: &AppState, user_id: &str) -> Option<String> {
    let query = "SELECT display_name FROM user_profiles WHERE user_id = $user_id";
    let user_id_owned = user_id.to_string();

    match state.db.query(query).bind(("user_id", user_id_owned)).await {
        Ok(mut response) => {
            #[derive(serde::Deserialize)]
            struct UserProfile {
                display_name: Option<String>,
            }

            let profiles: Vec<UserProfile> = response.take(0).unwrap_or_default();
            profiles.into_iter().next().and_then(|p| p.display_name)
        },
        Err(_) => None,
    }
}

/// Get user avatar URL from profile
async fn get_user_avatar_url(state: &AppState, user_id: &str) -> Option<String> {
    let query = "SELECT avatar_url FROM user_profiles WHERE user_id = $user_id";
    let user_id_owned = user_id.to_string();

    match state.db.query(query).bind(("user_id", user_id_owned)).await {
        Ok(mut response) => {
            #[derive(serde::Deserialize)]
            struct UserProfile {
                avatar_url: Option<String>,
            }

            let profiles: Vec<UserProfile> = response.take(0).unwrap_or_default();
            profiles.into_iter().next().and_then(|p| p.avatar_url)
        },
        Err(_) => None,
    }
}

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
