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
pub struct JoinRequest {
    /// Optional reason for joining
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Optional third-party signed token for invite validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub third_party_signed: Option<Value>,
}

#[derive(Serialize)]
pub struct JoinResponse {
    pub room_id: String,
}

/// Matrix Client-Server API v1.11 Section 10.2.4
///
/// POST /_matrix/client/v3/rooms/{roomId}/join
///
/// Join a room by room ID. This endpoint allows authenticated users to join
/// public rooms or rooms they have been invited to. Unlike the general join
/// endpoint, this one only accepts room IDs, not room aliases.
///
/// For public rooms, the user can join directly. For invite-only rooms,
/// the user must have a pending invitation.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(request): Json<JoinRequest>,
) -> Result<Json<JoinResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Room join failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room join failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room join failed - server authentication not allowed for room joins");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room join failed - anonymous authentication not allowed for room joins");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing room join request for user: {} to room: {} from: {}", user_id, room_id, addr);

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room join failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if user is already in the room
    if let Ok(current_membership) = get_user_membership(&state, &room_id, &user_id).await {
        match current_membership.membership {
            MembershipState::Join => {
                info!("User {} already joined room {}", user_id, room_id);
                return Ok(Json(JoinResponse { room_id }));
            },
            MembershipState::Ban => {
                warn!("Room join failed - user {} is banned from room {}", user_id, room_id);
                return Err(StatusCode::FORBIDDEN);
            },
            _ => {
                // User has some other membership state (invite, leave, knock) - proceed with join
            },
        }
    }

    // Get room information to check join rules
    let room: Option<Room> = state.db.select(("room", &room_id)).await.map_err(|e| {
        error!("Failed to query room {}: {}", room_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let room = room.ok_or_else(|| {
        warn!("Room join failed - room not found: {}", room_id);
        StatusCode::NOT_FOUND
    })?;

    // Check join authorization based on room join rules
    if !can_user_join(&state, &room, &user_id).await? {
        warn!("Room join failed - user {} not authorized to join room {}", user_id, room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Create join event
    let event_depth = get_next_event_depth(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let prev_events = get_latest_event_ids(&state, &room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_events = get_auth_events_for_join(&state, &room_id, &user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let join_event_id = create_membership_event(
        &state,
        &room_id,
        &user_id,
        &user_id,
        MembershipState::Join,
        request.reason.as_deref(),
        event_depth,
        &prev_events,
        &auth_events,
    )
    .await
    .map_err(|e| {
        error!("Failed to create join event for user {} in room {}: {}", user_id, room_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Successfully created join event {} for user {} in room {}",
        join_event_id, user_id, room_id
    );

    Ok(Json(JoinResponse { room_id }))
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

async fn can_user_join(state: &AppState, room: &Room, user_id: &str) -> Result<bool, StatusCode> {
    match room.join_rules.as_deref() {
        Some("public") => Ok(true),
        Some("invite") => {
            // Check if user has pending invitation
            match get_user_membership(state, &room.room_id, user_id).await {
                Ok(membership) => Ok(membership.membership == MembershipState::Invite),
                Err(_) => Ok(false),
            }
        },
        Some("knock") => {
            // Check if user has sent a knock request
            match get_user_membership(state, &room.room_id, user_id).await {
                Ok(membership) => Ok(membership.membership == MembershipState::Knock),
                Err(_) => Ok(false),
            }
        },
        Some("restricted") => {
            // MSC3083: Restricted room access rules
            // First check for pending invite
            if let Ok(membership) = get_user_membership(state, &room.room_id, user_id).await {
                if membership.membership == MembershipState::Invite {
                    return Ok(true);
                }
            }

            // Check allow conditions from room's join_rules state event
            let allow_conditions = get_room_join_rule_allow_conditions(state, &room.room_id)
                .await
                .map_err(|e| {
                    error!(
                        "Failed to get join rule allow conditions for room {}: {}",
                        room.room_id, e
                    );
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            // Check if user is a member of any of the allowed rooms/spaces
            for condition in allow_conditions {
                if condition.get("type").and_then(|v| v.as_str()) == Some("m.room_membership") {
                    if let Some(allowed_room_id) = condition.get("room_id").and_then(|v| v.as_str())
                    {
                        if let Ok(membership) =
                            get_user_membership(state, allowed_room_id, user_id).await
                        {
                            if membership.membership == MembershipState::Join {
                                info!(
                                    "User {} allowed to join restricted room {} via membership in room {}",
                                    user_id, room.room_id, allowed_room_id
                                );
                                return Ok(true);
                            }
                        }
                    }
                }
            }
            Ok(false)
        },
        Some("private") | _ => Ok(false), // Private or unknown join rule
    }
}

async fn get_room_join_rule_allow_conditions(
    state: &AppState,
    room_id: &str,
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>> {
    let query = "
        SELECT content
        FROM room_state_events
        WHERE room_id = $room_id 
          AND event_type = 'm.room.join_rules'
          AND state_key = ''
        ORDER BY origin_server_ts DESC
        LIMIT 1
    ";

    let mut response = state
        .db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .await
        .map_err(|e| format!("Database query failed for join rules: {}", e))?;

    let content: Option<Value> = response
        .take(0)
        .map_err(|e| format!("Failed to parse join rules query result: {}", e))?;

    match content {
        Some(content_value) => {
            let allow_conditions = content_value
                .get("allow")
                .and_then(|v| v.as_array())
                .unwrap_or(&vec![])
                .clone();
            Ok(allow_conditions)
        },
        None => Ok(vec![]),
    }
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

async fn get_auth_events_for_join(
    state: &AppState,
    room_id: &str,
    _user_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let query = r#"
        SELECT event_id FROM event 
        WHERE room_id = $room_id 
        AND event_type IN ['m.room.create', 'm.room.join_rules', 'm.room.power_levels']
        AND state_key = ''
        ORDER BY origin_server_ts ASC
    "#;

    let mut result = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

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

    // Get user profile information
    let display_name = get_user_display_name(state, target).await;
    let avatar_url = get_user_avatar_url(state, target).await;
    let is_direct = is_direct_message_room(state, room_id).await.unwrap_or(false);

    // Create/update membership record
    let membership_record = Membership {
        user_id: target.to_string(),
        room_id: room_id.to_string(),
        membership: membership.clone(),
        reason: reason.map(|r| r.to_string()),
        invited_by: if membership == MembershipState::Invite && sender != target {
            Some(sender.to_string())
        } else {
            None
        },
        updated_at: Some(Utc::now()),
        display_name,
        avatar_url,
        is_direct: Some(is_direct),
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

async fn get_user_display_name(state: &AppState, user_id: &str) -> Option<String> {
    let query = "SELECT display_name FROM user_profiles WHERE user_id = $user_id";
    match state.db.query(query).bind(("user_id", user_id.to_string())).await {
        Ok(mut response) => {
            let display_name: Option<String> = response.take(0).ok()?;
            display_name
        },
        Err(_) => None,
    }
}

async fn get_user_avatar_url(state: &AppState, user_id: &str) -> Option<String> {
    let query = "SELECT avatar_url FROM user_profiles WHERE user_id = $user_id";
    match state.db.query(query).bind(("user_id", user_id.to_string())).await {
        Ok(mut response) => {
            let avatar_url: Option<String> = response.take(0).ok()?;
            avatar_url
        },
        Err(_) => None,
    }
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
