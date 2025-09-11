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
pub struct UnbanRequest {
    /// The Matrix user ID of the user to unban
    pub user_id: String,

    /// Optional reason for the unban
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct UnbanResponse {
    // Empty response body per Matrix specification
}

/// Matrix Client-Server API v1.11 Section 10.2.7
///
/// POST /_matrix/client/v3/rooms/{roomId}/unban
///
/// Remove a ban from a user in a room. The authenticated user must have sufficient
/// power level to unban users in the room. The unbanned user will be able to join
/// the room again if they have appropriate permissions.
///
/// This creates a leave membership event for the banned user, effectively
/// removing the ban but not automatically adding them back to the room.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<UnbanRequest>,
) -> Result<Json<UnbanResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Room unban failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let unbanner_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room unban failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room unban failed - server authentication not allowed for room unbans");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room unban failed - anonymous authentication not allowed for room unbans");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room unban request from user: {} to unban: {} from room: {} (from: {})",
        unbanner_id, request.user_id, room_id, addr
    );

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room unban failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate user ID format
    if !request.user_id.starts_with('@') || !request.user_id.contains(':') {
        warn!("Room unban failed - invalid user ID format: {}", request.user_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Cannot unban yourself
    if unbanner_id == request.user_id {
        warn!("Room unban failed - user {} cannot unban themselves", unbanner_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if room exists
    let room: Option<Room> = state.db.select(("room", &room_id)).await.map_err(|e| {
        error!("Failed to query room {}: {}", room_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let room = room.ok_or_else(|| {
        warn!("Room unban failed - room not found: {}", room_id);
        StatusCode::NOT_FOUND
    })?;

    // Check if unbanner is a member of the room with appropriate permissions
    let unbanner_membership = get_user_membership(&state, &room_id, &unbanner_id).await.map_err(|_| {
        warn!("Room unban failed - unbanner {} is not a member of room {}", unbanner_id, room_id);
        StatusCode::FORBIDDEN
    })?;

    if unbanner_membership.membership != MembershipState::Join {
        warn!(
            "Room unban failed - unbanner {} must be joined to room {} to unban users",
            unbanner_id, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Check target user's current membership - must be banned to unban
    let target_membership = get_user_membership(&state, &room_id, &request.user_id).await.map_err(|_| {
        warn!("Room unban failed - target user {} has no membership in room {}", request.user_id, room_id);
        StatusCode::BAD_REQUEST
    })?;

    if target_membership.membership != MembershipState::Ban {
        match target_membership.membership {
            MembershipState::Join => {
                warn!("User {} is joined to room {} and not banned", request.user_id, room_id);
                return Err(StatusCode::BAD_REQUEST);
            },
            MembershipState::Leave => {
                info!("User {} is not banned from room {} (already left)", request.user_id, room_id);
                return Ok(Json(UnbanResponse {}));
            },
            MembershipState::Invite => {
                warn!("User {} is invited to room {} and not banned", request.user_id, room_id);
                return Err(StatusCode::BAD_REQUEST);
            },
            MembershipState::Knock => {
                warn!("User {} is knocking on room {} and not banned", request.user_id, room_id);
                return Err(StatusCode::BAD_REQUEST);
            },
            MembershipState::Ban => {
                // This case is handled by the outer if condition, but included for exhaustiveness
                unreachable!("Ban state should be handled by outer condition");
            },
        }
    }

    // Check unbanner's power level and permission to unban
    if !can_user_unban(&state, &room, &unbanner_id, &request.user_id).await? {
        warn!(
            "Room unban failed - user {} does not have permission to unban {} in room {}",
            unbanner_id, request.user_id, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    // Get event context for the unban event (which is actually a leave event)
    let event_depth = get_next_event_depth(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let prev_events = get_latest_event_ids(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_events = get_auth_events_for_unban(&state, &room_id, &unbanner_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create leave membership event for the unbanned user
    let unban_event_id = create_membership_event(
        &state,
        &room_id,
        &unbanner_id,         // The unbanner is the sender
        &request.user_id,     // The unbanned user is the target
        MembershipState::Leave, // Unban results in leave membership
        request.reason.as_deref(),
        event_depth,
        &prev_events,
        &auth_events,
    )
    .await
    .map_err(|e| {
        error!(
            "Failed to create unban event from {} to unban {} in room {}: {}",
            unbanner_id, request.user_id, room_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Successfully created unban event {} from {} to unban {} in room {}",
        unban_event_id, unbanner_id, request.user_id, room_id
    );

    Ok(Json(UnbanResponse {}))
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

async fn can_user_unban(
    state: &AppState,
    room: &Room,
    unbanner_id: &str,
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

    // Get the unbanner's power level
    let unbanner_power_level =
        if let Some(users) = power_levels.get("users").and_then(|u| u.as_object()) {
            users.get(unbanner_id).and_then(|p| p.as_i64()).unwrap_or(0)
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

    // Get the required power level to ban (unban uses same level as ban, default 50)
    let required_ban_level = power_levels.get("ban").and_then(|b| b.as_i64()).unwrap_or(50);

    // Unbanner must have sufficient power level to ban (unban) AND must have higher power level than target
    Ok(unbanner_power_level >= required_ban_level && unbanner_power_level > target_power_level)
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

async fn get_auth_events_for_unban(
    state: &AppState,
    room_id: &str,
    unbanner_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get the auth events needed for an unban event:
    // - m.room.create event
    // - m.room.power_levels event
    // - Unbanning user's m.room.member event (join state)

    let query = r#"
        SELECT event_id FROM event 
        WHERE room_id = $room_id 
        AND ((event_type = 'm.room.create' AND state_key = '')
             OR (event_type = 'm.room.power_levels' AND state_key = '')
             OR (event_type = 'm.room.member' AND state_key = $unbanner_id))
        ORDER BY origin_server_ts ASC
    "#;

    let mut result = state
        .db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .bind(("unbanner_id", unbanner_id.to_string()))
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
    let existing_membership: Option<Membership> = state.db.select(("membership", &membership_id)).await.ok().flatten();

    // Create/update membership record
    let membership_record = Membership {
        user_id: target.to_string(),
        room_id: room_id.to_string(),
        membership: membership.clone(),
        reason: reason.map(|r| r.to_string()),
        invited_by: None, // Not applicable for unban events
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
