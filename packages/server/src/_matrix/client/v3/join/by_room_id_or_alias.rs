use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, Path, State},
    http::{HeaderMap, StatusCode},
};
use base64::{Engine, engine::general_purpose};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use sha2::{Digest, Sha256};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
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

/// Matrix Client-Server API v1.11 Section 10.2.1
///
/// POST /_matrix/client/v3/join/{roomIdOrAlias}
///
/// Join a room by room ID or room alias. This endpoint allows authenticated
/// users to join public rooms or rooms they have been invited to.
///
/// For public rooms, the user can join directly. For invite-only rooms,
/// the user must have a pending invitation.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(room_id_or_alias): Path<String>,
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

    info!(
        "Processing room join request for user: {} to room: {} from: {}",
        user_id, room_id_or_alias, addr
    );

    // Resolve room ID from alias if necessary
    let actual_room_id = if room_id_or_alias.starts_with('#') {
        // Room alias - need to resolve to room ID
        resolve_room_alias(&state, &room_id_or_alias).await.map_err(|_| {
            warn!("Room join failed - could not resolve room alias: {}", room_id_or_alias);
            StatusCode::NOT_FOUND
        })?
    } else if room_id_or_alias.starts_with('!') {
        // Already a room ID
        room_id_or_alias.clone()
    } else {
        warn!("Room join failed - invalid room identifier format: {}", room_id_or_alias);
        return Err(StatusCode::BAD_REQUEST);
    };

    // Check if user is already in the room
    if let Ok(current_membership) = get_user_membership(&state, &actual_room_id, &user_id).await {
        match current_membership.membership {
            MembershipState::Join => {
                info!("User {} already joined room {}", user_id, actual_room_id);
                return Ok(Json(JoinResponse { room_id: actual_room_id }));
            },
            MembershipState::Ban => {
                warn!("Room join failed - user {} is banned from room {}", user_id, actual_room_id);
                return Err(StatusCode::FORBIDDEN);
            },
            _ => {
                // User has some other membership state (invite, leave, knock) - proceed with join
            },
        }
    }

    // Get room information to check join rules
    let room: Option<Room> = state.db.select(("room", &actual_room_id)).await.map_err(|e| {
        error!("Failed to query room {}: {}", actual_room_id, e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let room = room.ok_or_else(|| {
        warn!("Room join failed - room not found: {}", actual_room_id);
        StatusCode::NOT_FOUND
    })?;

    // Check join authorization based on room join rules
    if !can_user_join(&state, &room, &user_id).await? {
        warn!("Room join failed - user {} not authorized to join room {}", user_id, actual_room_id);
        return Err(StatusCode::FORBIDDEN);
    }

    // Create join event
    let event_depth = get_next_event_depth(&state, &actual_room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let prev_events = get_latest_event_ids(&state, &actual_room_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let auth_events = get_auth_events_for_join(&state, &actual_room_id, &user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let join_event_id = create_membership_event(
        &state,
        &actual_room_id,
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
        error!(
            "Failed to create join event for user {} in room {}: {}",
            user_id, actual_room_id, e
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!(
        "Successfully created join event {} for user {} in room {}",
        join_event_id, user_id, actual_room_id
    );

    Ok(Json(JoinResponse { room_id: actual_room_id }))
}

async fn resolve_room_alias(
    state: &AppState,
    alias: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Query the room_aliases table to find the room_id for the given alias
    let query = "SELECT room_id FROM room_aliases WHERE alias = $alias";

    let mut response = state
        .db
        .query(query)
        .bind(("alias", alias.to_string()))
        .await
        .map_err(|e| format!("Database query failed for room alias resolution: {}", e))?;

    // Take the first result if available
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
            // A user can join a restricted room if:
            // 1. They have a pending invite, OR
            // 2. They are a member of at least one of the rooms/spaces listed in the allow conditions

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
                        // Check if user is a member of this allowed room
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

            // User does not meet any of the restricted access conditions
            Ok(false)
        },
        Some("private") | _ => Ok(false), // Private or unknown join rule
    }
}

async fn get_room_join_rule_allow_conditions(
    state: &AppState,
    room_id: &str,
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>> {
    // Query the room's join_rules state event to get allow conditions
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
            // Extract allow conditions from the join_rules content
            let allow_conditions = content_value
                .get("allow")
                .and_then(|v| v.as_array())
                .unwrap_or(&vec![])
                .clone();

            Ok(allow_conditions)
        },
        None => {
            // No join_rules event found, default to empty allow list
            Ok(vec![])
        },
    }
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

async fn get_auth_events_for_join(
    state: &AppState,
    room_id: &str,
    _user_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get the auth events needed for a join event:
    // - m.room.create event
    // - m.room.join_rules event
    // - m.room.power_levels event
    // - Inviting user's m.room.member event (if joining via invite)

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

    // Create the event with initial structure
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
    let hashes_value = calculate_content_hashes(&event).map_err(|e| {
        error!("Failed to calculate content hashes: {}", e);
        e
    })?;
    let hashes: HashMap<String, String> = serde_json::from_value(hashes_value).map_err(|e| {
        error!("Failed to convert hashes: {}", e);
        e
    })?;
    event.hashes = Some(hashes);

    // Sign event with server's Ed25519 private key
    let signatures_value = sign_event(state, &event).await.map_err(|e| {
        error!("Failed to sign event: {}", e);
        e
    })?;
    let signatures: HashMap<String, HashMap<String, String>> = serde_json::from_value(signatures_value).map_err(|e| {
        error!("Failed to convert signatures: {}", e);
        e
    })?;
    event.signatures = Some(signatures);

    // Store the event
    let _: Option<Event> = state.db.create(("event", &event_id)).content(event).await?;

    // Get user profile information for membership record
    let display_name = get_user_display_name(state, target)
        .await
        .map_err(|e| {
            warn!("Failed to get display name for user {}: {}", target, e);
            e
        })
        .ok()
        .flatten();

    let avatar_url = get_user_avatar_url(state, target)
        .await
        .map_err(|e| {
            warn!("Failed to get avatar URL for user {}: {}", target, e);
            e
        })
        .ok()
        .flatten();

    // Determine if this is a direct message room
    let is_direct = is_direct_message_room(state, room_id)
        .await
        .map_err(|e| {
            warn!("Failed to determine if room {} is direct: {}", room_id, e);
            e
        })
        .unwrap_or(false);

    // Create/update membership record with profile information
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
        .create(("membership", membership_id))
        .content(membership_record)
        .await?;

    Ok(event_id)
}

/// Calculate SHA256 content hashes for Matrix event according to specification
///
/// Creates canonical JSON representation of event content and calculates
/// SHA256 hash following Matrix specification requirements for event integrity.
fn calculate_content_hashes(
    event: &Event,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    // Create canonical JSON for content hashing per Matrix specification
    let canonical_content = json!({
        "auth_events": event.auth_events,
        "content": event.content,
        "depth": event.depth,
        "event_type": event.event_type,
        "prev_events": event.prev_events,
        "room_id": event.room_id,
        "sender": event.sender,
        "state_key": event.state_key,
        "origin_server_ts": event.origin_server_ts
    });

    // Convert to canonical JSON string (sorted keys, no whitespace)
    let canonical_json = to_canonical_json(&canonical_content)?;

    // Calculate SHA256 hash
    let mut hasher = Sha256::new();
    hasher.update(canonical_json.as_bytes());
    let hash = hasher.finalize();

    // Encode as base64
    let hash_b64 = general_purpose::STANDARD.encode(&hash);

    Ok(json!({
        "sha256": hash_b64
    }))
}

/// Sign Matrix event with server's Ed25519 private key according to specification
///
/// Creates canonical JSON representation and signs with server's private key
/// following Matrix federation signature requirements for event authentication.
async fn sign_event(
    state: &AppState,
    event: &Event,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    use ed25519_dalek::{Signer, SigningKey};

    // Get server signing key from database
    let query = "
        SELECT private_key, key_id 
        FROM server_signing_keys 
        WHERE server_name = $server_name 
          AND is_active = true 
        ORDER BY created_at DESC 
        LIMIT 1
    ";

    let mut response = state
        .db
        .query(query)
        .bind(("server_name", state.homeserver_name.clone()))
        .await?;

    #[derive(serde::Deserialize)]
    struct SigningKeyRecord {
        private_key: String,
        key_id: String,
    }

    let key_record: Option<SigningKeyRecord> = response.take(0)?;
    let key_record = key_record.ok_or("No active signing key found for server")?;

    // Create canonical JSON for signing per Matrix specification
    let canonical_event = json!({
        "auth_events": event.auth_events,
        "content": event.content,
        "depth": event.depth,
        "event_type": event.event_type,
        "hashes": event.hashes,
        "prev_events": event.prev_events,
        "room_id": event.room_id,
        "sender": event.sender,
        "state_key": event.state_key,
        "origin_server_ts": event.origin_server_ts
    });

    // Convert to canonical JSON string
    let canonical_json = to_canonical_json(&canonical_event)?;

    // Decode private key from base64
    let private_key_bytes = general_purpose::STANDARD.decode(&key_record.private_key)?;

    // Validate key length
    if private_key_bytes.len() != 32 {
        return Err("Invalid private key length for Ed25519".into());
    }

    // Create Ed25519 signing key
    let private_key_array: [u8; 32] = private_key_bytes
        .try_into()
        .map_err(|_| "Failed to convert private key to array")?;
    let signing_key = SigningKey::from_bytes(&private_key_array);

    // Sign canonical JSON
    let signature = signing_key.sign(canonical_json.as_bytes());
    let signature_b64 = general_purpose::STANDARD.encode(signature.to_bytes());

    Ok(json!({
        state.homeserver_name.clone(): {
            key_record.key_id: signature_b64
        }
    }))
}

/// Get user display name from profile data
///
/// Queries user profile table to retrieve the current display name
/// for the specified user ID, returning None if not found.
async fn get_user_display_name(
    state: &AppState,
    user_id: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let query = "SELECT display_name FROM user_profiles WHERE user_id = $user_id";
    let mut response = state.db.query(query).bind(("user_id", user_id.to_string())).await?;

    let display_name: Option<String> = response.take(0)?;
    Ok(display_name)
}

/// Get user avatar URL from profile data
///
/// Queries user profile table to retrieve the current avatar URL
/// for the specified user ID, returning None if not found.
async fn get_user_avatar_url(
    state: &AppState,
    user_id: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let query = "SELECT avatar_url FROM user_profiles WHERE user_id = $user_id";
    let mut response = state.db.query(query).bind(("user_id", user_id.to_string())).await?;

    let avatar_url: Option<String> = response.take(0)?;
    Ok(avatar_url)
}

/// Determine if room represents a direct message conversation
///
/// Analyzes room properties and membership to determine if this is
/// a direct message room (typically 2 members, no room name/topic).
async fn is_direct_message_room(
    state: &AppState,
    room_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Query member count in room
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

    // Check if room has explicit name or topic (indicates not a DM)
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

    // Consider it a direct message if:
    // - Exactly 2 members
    // - No explicit room name or topic
    Ok(member_count == 2 && !has_name_or_topic)
}

/// Convert JSON value to Matrix canonical JSON string with sorted keys
///
/// Implements Matrix canonical JSON as defined in the Matrix specification:
/// - Object keys sorted in lexicographic order
/// - No insignificant whitespace
/// - UTF-8 encoding
/// - Numbers in shortest form
///
/// This is critical for signature verification and hash calculation to work
/// correctly with other Matrix homeservers.
fn to_canonical_json(value: &Value) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    match value {
        Value::Null => Ok("null".to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => Ok(n.to_string()),
        Value::String(s) => {
            // JSON string with proper escaping
            Ok(serde_json::to_string(s)?)
        },
        Value::Array(arr) => {
            let elements: Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> =
                arr.iter().map(|v| to_canonical_json(v)).collect();
            Ok(format!("[{}]", elements?.join(",")))
        },
        Value::Object(obj) => {
            // Sort keys lexicographically (critical for Matrix signature verification)
            let mut sorted_keys: Vec<&String> = obj.keys().collect();
            sorted_keys.sort();

            let pairs: Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> = sorted_keys
                .into_iter()
                .map(|key| {
                    let key_json = serde_json::to_string(key)?;
                    let value_json = to_canonical_json(&obj[key])?;
                    Ok(format!("{}:{}", key_json, value_json))
                })
                .collect();

            Ok(format!("{{{}}}", pairs?.join(",")))
        },
    }
}
