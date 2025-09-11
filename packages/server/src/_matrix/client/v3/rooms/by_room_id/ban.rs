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
pub struct BanRequest {
    /// The Matrix user ID of the user to ban
    pub user_id: String,

    /// Optional reason for the ban
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct BanResponse {
    // Empty response body per Matrix specification
}

/// Matrix Client-Server API v1.11 Section 10.2.5
///
/// POST /_matrix/client/v3/rooms/{roomId}/ban
///
/// Ban a user from a room. The authenticated user must have sufficient power
/// level to ban users in the room. The banned user will be immediately removed
/// from the room and will not be able to join until they are unbanned.
///
/// This endpoint can be used to ban users who are currently in the room or
/// users who are not currently in the room (preemptive ban).
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<BanRequest>,
) -> Result<Json<BanResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Room ban failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let banner_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room ban failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room ban failed - server authentication not allowed for room bans");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room ban failed - anonymous authentication not allowed for room bans");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room ban request from user: {} to ban: {} from room: {} (from: {})",
        banner_id, request.user_id, room_id, addr
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room ban failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate user ID format
    if !request.user_id.starts_with('@') || !request.user_id.contains(':') {
        warn!("Room ban failed - invalid user ID format: {}", request.user_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Cannot ban yourself
    if banner_id == request.user_id {
        warn!("Room ban failed - user {} cannot ban themselves", banner_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if room exists
    let room: Option<Room> = state.db.select(("room", &room_id)).await.map_err(|e| {
        error!("Failed to query room {}: {}", room_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let room = room.ok_or_else(|| {
        warn!("Room ban failed - room not found: {}", room_id);
        StatusCode::NOT_FOUND
    })?;

    // Check if banner is a member of the room with appropriate permissions
    let banner_membership =
        get_user_membership(&state, &room_id, &banner_id).await.map_err(|_| {
            warn!("Room ban failed - banner {} is not a member of room {}", banner_id, room_id);
            StatusCode::FORBIDDEN
        })?;

    if banner_membership.membership != MembershipState::Join {
        warn!(
            "Room ban failed - banner {} must be joined to room {} to ban users",
            banner_id, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Check banner's power level and permission to ban
    if !can_user_ban(&state, &room, &banner_id, &request.user_id).await? {
        warn!(
            "Room ban failed - user {} does not have permission to ban {} in room {}",
            banner_id, request.user_id, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Check if user is already banned
    if let Ok(existing_membership) = get_user_membership(&state, &room_id, &request.user_id).await {
        if existing_membership.membership == MembershipState::Ban {
            info!("User {} is already banned from room {}", request.user_id, room_id);
            return Ok(Json(BanResponse {}));
        }
    }

    // Get event context for the ban event
    let event_depth = get_next_event_depth(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let prev_events = get_latest_event_ids(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_events = get_auth_events_for_ban(&state, &room_id, &banner_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create ban membership event
    let ban_event_id = create_membership_event(
        &state,
        &room_id,
        &banner_id,
        &request.user_id,
        MembershipState::Ban,
        request.reason.as_deref(),
        event_depth,
        &prev_events,
        &auth_events,
    )
    .await
    .map_err(|e| {
        error!(
            "Failed to create ban event from {} to ban {} in room {}: {}",
            banner_id, request.user_id, room_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Successfully created ban event {} from {} to ban {} in room {}",
        ban_event_id, banner_id, request.user_id, room_id
    );

    Ok(Json(BanResponse {}))
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

async fn can_user_ban(
    state: &AppState,
    room: &Room,
    banner_id: &str,
    target_id: &str,
) -> Result<bool, StatusCode> {
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

    // Get the banner's power level
    let banner_power_level =
        if let Some(users) = power_levels.get("users").and_then(|u| u.as_object()) {
            users.get(banner_id).and_then(|p| p.as_i64()).unwrap_or(0)
        } else {
            0
        };

    // Get the target's power level
    let target_power_level =
        if let Some(users) = power_levels.get("users").and_then(|u| u.as_object()) {
            users.get(target_id).and_then(|p| p.as_i64()).unwrap_or(0)
        } else {
            0
        };

    // Get the required power level to ban (default 50 per Matrix spec)
    let required_ban_level = power_levels.get("ban").and_then(|b| b.as_i64()).unwrap_or(50);

    // Banner must have sufficient power level to ban AND must have higher power level than target
    Ok(banner_power_level >= required_ban_level && banner_power_level > target_power_level)
}

async fn get_next_event_depth(
    state: &AppState,
    room_id: &str,
) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
    let query = "SELECT VALUE math::max(depth) FROM event WHERE room_id = $room_id";
    let mut result = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    let max_depth: Option<i64> = result.take(0)?;
    Ok(max_depth.unwrap_or(0) + 1)
}

async fn get_latest_event_ids(
    state: &AppState,
    room_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let query = "SELECT event_id FROM event WHERE room_id = $room_id ORDER BY depth DESC LIMIT 3";
    let mut result = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    let events: Vec<String> = result.take(0)?;
    Ok(events)
}

async fn get_auth_events_for_ban(
    state: &AppState,
    room_id: &str,
    banner_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get the auth events needed for a ban event:
    // - m.room.create event
    // - m.room.power_levels event
    // - Banning user's m.room.member event (join state)

    let query = r#"
        SELECT event_id FROM event 
        WHERE room_id = $room_id 
        AND ((event_type = 'm.room.create' AND state_key = '')
             OR (event_type = 'm.room.power_levels' AND state_key = '')
             OR (event_type = 'm.room.member' AND state_key = $banner_id))
        ORDER BY origin_server_ts ASC
    "#;

    let mut result = state
        .db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .bind(("banner_id", banner_id.to_string()))
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

    // Calculate content hashes
    let hashes_value = calculate_content_hashes(&event)?;
    let hashes: HashMap<String, String> = serde_json::from_value(hashes_value)?;
    event.hashes = Some(hashes);

    // Sign event
    let signatures_value = sign_event(state, &event).await?;
    let signatures: HashMap<String, HashMap<String, String>> =
        serde_json::from_value(signatures_value)?;
    event.signatures = Some(signatures);

    // Store the event
    let _: Option<Event> = state.db.create(("event", &event_id)).content(event).await?;

    // Get existing membership to preserve display name and avatar
    let membership_id = format!("{}:{}", target, room_id);
    let existing_membership: Option<Membership> =
        state.db.select(("membership", &membership_id)).await.ok().flatten();

    // Create/update membership record
    let membership_record = Membership {
        user_id: target.to_string(),
        room_id: room_id.to_string(),
        membership: membership.clone(),
        reason: reason.map(|r| r.to_string()),
        invited_by: None, // Not applicable for ban events
        updated_at: Some(Utc::now()),
        display_name: existing_membership.as_ref().and_then(|m| m.display_name.clone()),
        avatar_url: existing_membership.as_ref().and_then(|m| m.avatar_url.clone()),
        is_direct: Some(is_direct_message_room(state, room_id).await.unwrap_or(false)),
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

async fn is_direct_message_room(
    state: &AppState,
    room_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let member_count_query = "
        SELECT count() 
        FROM membership 
        WHERE room_id = $room_id 
          AND membership = 'join'
    ";

    let mut response = state
        .db
        .query(member_count_query)
        .bind(("room_id", room_id.to_string()))
        .await?;

    let member_count: Option<i64> = response.take(0)?;
    let member_count = member_count.unwrap_or(0);

    let room_state_query = "
        SELECT count()
        FROM event 
        WHERE room_id = $room_id 
          AND event_type IN ['m.room.name', 'm.room.topic']
          AND state_key = ''
    ";

    let mut response = state
        .db
        .query(room_state_query)
        .bind(("room_id", room_id.to_string()))
        .await?;

    let has_name_or_topic: Option<i64> = response.take(0)?;
    let has_name_or_topic = has_name_or_topic.unwrap_or(0) > 0;

    Ok(member_count == 2 && !has_name_or_topic)
}
