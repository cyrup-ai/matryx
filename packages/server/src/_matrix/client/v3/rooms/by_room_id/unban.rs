use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
    utils::matrix_events::{calculate_content_hashes, sign_event},
};
use matryx_entity::types::{Event, EventContent, Membership, MembershipState, Room};
use matryx_surrealdb::repository::{EventRepository, MembershipRepository, RoomRepository};
use std::sync::Arc;

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
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
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

    // Check if room exists using repository
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let room = room_repo.get_by_id(&room_id).await.map_err(|e| {
        error!("Failed to query room {}: {}", room_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?.ok_or_else(|| {
        warn!("Room unban failed - room not found: {}", room_id);
        StatusCode::NOT_FOUND
    })?;

    // Check if unbanner is a member of the room with appropriate permissions
    let unbanner_membership =
        get_user_membership(&state, &room_id, &unbanner_id).await.map_err(|_| {
            warn!(
                "Room unban failed - unbanner {} is not a member of room {}",
                unbanner_id, room_id
            );
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
    let target_membership =
        get_user_membership(&state, &room_id, &request.user_id)
            .await
            .map_err(|_| {
                warn!(
                    "Room unban failed - target user {} has no membership in room {}",
                    request.user_id, room_id
                );
                StatusCode::BAD_REQUEST
            })?;

    if target_membership.membership != MembershipState::Ban {
        match target_membership.membership {
            MembershipState::Join => {
                warn!("User {} is joined to room {} and not banned", request.user_id, room_id);
                return Err(StatusCode::BAD_REQUEST);
            },
            MembershipState::Leave => {
                info!(
                    "User {} is not banned from room {} (already left)",
                    request.user_id, room_id
                );
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
    let unban_event_id = create_membership_event(MembershipEventParams {
        state: &state,
        room_id: &room_id,
        sender: &unbanner_id,           // The unbanner is the sender
        target: &request.user_id,       // The unbanned user is the target
        membership: MembershipState::Leave, // Unban results in leave membership
        reason: request.reason.as_deref(),
        depth: event_depth,
        prev_events: &prev_events,
        auth_events: &auth_events,
    })
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
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let membership = membership_repo.get_membership(room_id, user_id).await?;

    membership.ok_or_else(|| "Membership not found".into())
}

async fn can_user_unban(
    _state: &AppState,
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
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let depth = event_repo.get_next_event_depth(room_id).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(depth)
}

async fn get_latest_event_ids(
    state: &AppState,
    room_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let event_ids = event_repo.get_latest_event_ids(room_id, 3).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(event_ids)
}

async fn get_auth_events_for_unban(
    state: &AppState,
    room_id: &str,
    unbanner_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));
    let auth_events = event_repo.get_auth_events_for_unban(room_id, unbanner_id).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(auth_events)
}

struct MembershipEventParams<'a> {
    state: &'a AppState,
    room_id: &'a str,
    sender: &'a str,
    target: &'a str,
    membership: MembershipState,
    reason: Option<&'a str>,
    depth: i64,
    prev_events: &'a [String],
    auth_events: &'a [String],
}

async fn create_membership_event(
    params: MembershipEventParams<'_>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let event_id = format!("${}:{}", Uuid::new_v4(), params.state.homeserver_name);

    let mut content = json!({
        "membership": match params.membership {
            MembershipState::Join => "join",
            MembershipState::Leave => "leave",
            MembershipState::Invite => "invite",
            MembershipState::Ban => "ban",
            MembershipState::Knock => "knock",
        }
    });

    if let Some(reason) = params.reason {
        content["reason"] = json!(reason);
    }

    // Create the event
    let mut event = Event {
        event_id: event_id.clone(),
        room_id: params.room_id.to_string(),
        sender: params.sender.to_string(),
        event_type: "m.room.member".to_string(),
        content: EventContent::Unknown(content.clone()),
        state_key: Some(params.target.to_string()),
        origin_server_ts: Utc::now().timestamp_millis(),
        unsigned: None,
        prev_events: Some(params.prev_events.to_vec()),
        auth_events: Some(params.auth_events.to_vec()),
        depth: Some(params.depth),
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
    let signatures_value = sign_event(params.state, &event).await?;
    let signatures: HashMap<String, HashMap<String, String>> =
        serde_json::from_value(signatures_value)?;
    event.signatures = Some(signatures);

    // Store the event using repository
    let event_repo = Arc::new(EventRepository::new(params.state.db.clone()));
    let _created_event = event_repo.create_room_event(
        params.room_id,
        &event.event_type,
        params.sender,
        serde_json::to_value(&event.content)?,
        event.state_key.clone(),
    ).await?;

    // Get existing membership to preserve display name and avatar
    let membership_repo = Arc::new(MembershipRepository::new(params.state.db.clone()));
    let existing_membership = membership_repo.get_membership(params.room_id, params.target).await.ok().flatten();

    // Check if room is direct message using repository
    let is_direct = event_repo.is_direct_message_room(params.room_id).await.unwrap_or(false);

    // Create/update membership record
    let membership_record = Membership {
        user_id: params.target.to_string(),
        room_id: params.room_id.to_string(),
        membership: params.membership.clone(),
        reason: params.reason.map(|r| r.to_string()),
        invited_by: None, // Not applicable for unban events
        updated_at: Some(Utc::now()),
        display_name: existing_membership.as_ref().and_then(|m| m.display_name.clone()),
        avatar_url: existing_membership.as_ref().and_then(|m| m.avatar_url.clone()),
        is_direct: Some(is_direct),
        third_party_invite: None,
        join_authorised_via_users_server: None,
    };

    membership_repo.upsert_membership_record(membership_record).await?;

    Ok(event_id)
}


