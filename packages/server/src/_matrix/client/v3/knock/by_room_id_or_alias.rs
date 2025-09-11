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
pub struct KnockRequest {
    /// Optional reason for knocking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize)]
pub struct KnockResponse {
    pub room_id: String,
}

/// Matrix Client-Server API MSC2403 - Knocking
///
/// POST /_matrix/client/v3/knock/{roomIdOrAlias}
///
/// Request to join a room that has knock permissions enabled. This sends a
/// knock membership event that room moderators can see and respond to by
/// inviting the user or ignoring the request.
///
/// Only works for rooms with join_rule set to "knock". Users cannot knock
/// on rooms they are already members of, invited to, or banned from.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id_or_alias): Path<String>,
    Json(request): Json<KnockRequest>,
) -> Result<Json<KnockResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Room knock failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room knock failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room knock failed - server authentication not allowed for room knocks");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room knock failed - anonymous authentication not allowed for room knocks");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!(
        "Processing room knock request for user: {} to room: {} from: {}",
        user_id, room_id_or_alias, addr
    );

    // Resolve room ID from alias if necessary
    let actual_room_id = if room_id_or_alias.starts_with('#') {
        // Room alias - need to resolve to room ID
        resolve_room_alias(&state, &room_id_or_alias).await.map_err(|_| {
            warn!("Room knock failed - could not resolve room alias: {}", room_id_or_alias);
            StatusCode::NOT_FOUND
        })?
    } else if room_id_or_alias.starts_with('!') {
        // Already a room ID
        room_id_or_alias.clone()
    } else {
        warn!("Room knock failed - invalid room identifier format: {}", room_id_or_alias);
        return Err(StatusCode::BAD_REQUEST);
    };

    // Check if user is already in the room
    if let Ok(current_membership) = get_user_membership(&state, &actual_room_id, &user_id).await {
        match current_membership.membership {
            MembershipState::Join => {
                warn!("User {} is already joined to room {}", user_id, actual_room_id);
                return Err(StatusCode::FORBIDDEN);
            },
            MembershipState::Invite => {
                warn!("User {} is already invited to room {}", user_id, actual_room_id);
                return Err(StatusCode::FORBIDDEN);
            },
            MembershipState::Ban => {
                warn!("Room knock failed - user {} is banned from room {}", user_id, actual_room_id);
                return Err(StatusCode::FORBIDDEN);
            },
            MembershipState::Knock => {
                info!("User {} has already knocked on room {}", user_id, actual_room_id);
                return Ok(Json(KnockResponse { room_id: actual_room_id }));
            },
            MembershipState::Leave => {
                // User previously left - can knock
            },
        }
    }

    // Get room information to check knock permissions
    let room: Option<Room> = state.db.select(("room", &actual_room_id)).await.map_err(|e| {
        error!("Failed to query room {}: {}", actual_room_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let room = room.ok_or_else(|| {
        warn!("Room knock failed - room not found: {}", actual_room_id);
        StatusCode::NOT_FOUND
    })?;

    // Check if room allows knocking
    match room.join_rules.as_deref() {
        Some("knock") => {
            // Room allows knocking - proceed
        },
        Some("public") => {
            warn!("Room knock failed - room {} is public, use join instead", actual_room_id);
            return Err(StatusCode::BAD_REQUEST);
        },
        Some("invite") => {
            warn!("Room knock failed - room {} is invite-only", actual_room_id);
            return Err(StatusCode::FORBIDDEN);
        },
        Some("restricted") | Some("private") | _ => {
            warn!("Room knock failed - room {} does not allow knocking", actual_room_id);
            return Err(StatusCode::FORBIDDEN);
        },
    }

    // Create knock event
    let event_depth = get_next_event_depth(&state, &actual_room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let prev_events = get_latest_event_ids(&state, &actual_room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_events = get_auth_events_for_knock(&state, &actual_room_id, &user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let knock_event_id = create_membership_event(
        &state,
        &actual_room_id,
        &user_id,
        &user_id,
        MembershipState::Knock,
        request.reason.as_deref(),
        event_depth,
        &prev_events,
        &auth_events,
    )
    .await
    .map_err(|e| {
        error!(
            "Failed to create knock event for user {} in room {}: {}",
            user_id, actual_room_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Successfully created knock event {} for user {} in room {}",
        knock_event_id, user_id, actual_room_id
    );

    Ok(Json(KnockResponse { room_id: actual_room_id }))
}

async fn resolve_room_alias(
    state: &AppState,
    alias: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let query = "SELECT room_id FROM room_aliases WHERE alias = $alias";

    let mut response = state
        .db
        .query(query)
        .bind(("alias", alias.to_string()))
        .await
        .map_err(|e| format!("Database query failed for room alias resolution: {}", e))?;

    let room_id: Option<String> = response
        .take(0)
        .map_err(|e| format!("Failed to parse room alias query result: {}", e))?;

    match room_id {
        Some(id) => {
            info!("Resolved room alias {} to room ID {}", alias, id);
            Ok(id)
        },
        None => {
            warn!("Room alias {} not found in database", alias);
            Err(format!("Room alias '{}' not found", alias).into())
        },
    }
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

async fn get_auth_events_for_knock(
    state: &AppState,
    room_id: &str,
    _user_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get the auth events needed for a knock event:
    // - m.room.create event
    // - m.room.join_rules event (to validate knocking is allowed)

    let query = r#"
        SELECT event_id FROM event 
        WHERE room_id = $room_id 
        AND event_type IN ['m.room.create', 'm.room.join_rules']
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
        invited_by: None, // Not applicable for knock events
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
