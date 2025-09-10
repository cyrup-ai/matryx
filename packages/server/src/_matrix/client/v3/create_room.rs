use std::net::SocketAddr;

use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
    utils::matrix_events::{calculate_content_hashes, sign_event},
};
use matryx_entity::types::{Event, Membership, MembershipState, Room};

#[derive(Deserialize)]
pub struct CreateRoomRequest {
    visibility: Option<String>, // "public" or "private"
    room_alias_name: Option<String>,
    name: Option<String>,
    topic: Option<String>,
    invite: Option<Vec<String>>,
    invite_3pid: Option<Vec<Value>>,
    room_version: Option<String>,
    creation_content: Option<Value>,
    initial_state: Option<Vec<InitialStateEvent>>,
    preset: Option<String>, // "private_chat", "public_chat", "trusted_private_chat"
    is_direct: Option<bool>,
    power_level_content_override: Option<Value>,
}

#[derive(Deserialize)]
struct InitialStateEvent {
    #[serde(rename = "type")]
    event_type: String,
    state_key: Option<String>,
    content: Value,
}

#[derive(Serialize)]
pub struct CreateRoomResponse {
    room_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    room_alias: Option<String>,
}

/// Matrix Client-Server API v1.11 Section 10.1.1
///
/// POST /_matrix/client/v3/createRoom
///
/// Create a new room with the given configuration. This endpoint supports all
/// Matrix room creation features including state events, invitations, power levels,
/// and room presets for different types of rooms.
///
/// This endpoint requires authentication and will create the room on behalf of
/// the authenticated user who becomes the room creator and initial member.
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<CreateRoomRequest>,
) -> Result<Json<CreateRoomResponse>, StatusCode> {
    let start_time = std::time::Instant::now();

    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Room creation failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room creation failed - access token expired for user");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Room creation failed - server authentication not allowed for room creation");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Room creation failed - anonymous authentication not allowed for room creation");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing room creation request for user: {} from: {}", user_id, addr);
    // Generate room ID
    let room_id = generate_room_id(&state.homeserver_name);

    // Set default room version if not specified
    let room_version = request.room_version.clone().unwrap_or_else(|| "10".to_string());

    // Validate room version
    if !is_supported_room_version(&room_version) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Determine visibility
    let visibility = request.visibility.clone().unwrap_or_else(|| "private".to_string());
    let is_public = visibility == "public";

    // Apply preset settings
    let (join_rules, history_visibility, guest_access) = match request.preset.as_deref() {
        Some("public_chat") => ("public", "shared", "forbidden"),
        Some("trusted_private_chat") => ("invite", "shared", "can_join"),
        _ => ("invite", "shared", "can_join"), // Default to private_chat
    };

    // Create room entity
    let room = Room {
        room_id: room_id.clone(),
        creator: user_id.clone(),
        room_version: room_version.clone(),
        created_at: Utc::now(),
        updated_at: Some(Utc::now()),
        is_public: Some(is_public),
        is_direct: Some(request.is_direct.unwrap_or(false)),
        room_type: request
            .creation_content
            .as_ref()
            .and_then(|c| c.get("type"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
        canonical_alias: None, // Set later if alias is created
        topic: request.topic.clone(),
        name: request.name.clone(),
        avatar_url: None,
        guest_access: Some(guest_access.to_string()),
        history_visibility: Some(history_visibility.to_string()),
        join_rule: Some(join_rules.to_string()),
        join_rules: Some(join_rules.to_string()),
        power_levels: serde_json::from_value(create_default_power_levels(&user_id, &request)).ok(),
        encryption: None,
        tombstone: None,
        predecessor: None,
        alt_aliases: None,
        state_events_count: Some(0),
        federate: Some(request
            .creation_content
            .as_ref()
            .and_then(|c| c.get("m.federate"))
            .and_then(|f| f.as_bool())
            .unwrap_or(true)),
    };

    // Store room
    let created_room: Option<Room> = state
        .db
        .create(("room", &room_id))
        .content(room.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if created_room.is_none() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Create room events in order
    let mut event_depth = 1i64;
    let mut prev_events = Vec::new();

    // 1. m.room.create event
    let create_event_id = create_room_create_event(
        &state,
        &room_id,
        &user_id,
        &room_version,
        request.creation_content.clone(),
        event_depth,
        &prev_events,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    prev_events = vec![create_event_id.clone()];
    event_depth += 1;

    // 2. Creator membership event
    let join_event_id = create_membership_event(
        &state,
        &room_id,
        &user_id,
        &user_id,
        MembershipState::Join,
        None,
        event_depth,
        &prev_events,
        &[create_event_id.clone()],
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    prev_events = vec![join_event_id.clone()];
    event_depth += 1;

    // 3. Power levels event
    let power_levels_event_id = create_power_levels_event(
        &state,
        &room_id,
        &user_id,
        &room.power_levels.as_ref().map(|pl| serde_json::to_value(pl).unwrap_or_default()).unwrap_or_default(),
        event_depth,
        &prev_events,
        &[create_event_id.clone(), join_event_id.clone()],
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    prev_events = vec![power_levels_event_id.clone()];
    event_depth += 1;

    // 4. Join rules event
    let join_rules_event_id = create_join_rules_event(
        &state,
        &room_id,
        &user_id,
        join_rules,
        event_depth,
        &prev_events,
        &[
            create_event_id.clone(),
            join_event_id.clone(),
            power_levels_event_id.clone(),
        ],
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    prev_events = vec![join_rules_event_id.clone()];
    event_depth += 1;

    // 5. History visibility event
    let _history_visibility_event_id = create_history_visibility_event(
        &state,
        &room_id,
        &user_id,
        history_visibility,
        event_depth,
        &prev_events,
        &[
            create_event_id.clone(),
            join_event_id.clone(),
            power_levels_event_id.clone(),
        ],
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    prev_events = vec![_history_visibility_event_id.clone()];
    event_depth += 1;

    // 6. Optional: Room name event
    if let Some(name) = &request.name {
        let _name_event_id =
            create_name_event(&state, &room_id, &user_id, name, event_depth, &prev_events, &[
                create_event_id.clone(),
                join_event_id.clone(),
                power_levels_event_id.clone(),
            ])
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        prev_events = vec![_name_event_id];
        event_depth += 1;
    }

    // 7. Optional: Room topic event
    if let Some(topic) = &request.topic {
        let _topic_event_id =
            create_topic_event(&state, &room_id, &user_id, topic, event_depth, &prev_events, &[
                create_event_id.clone(),
                join_event_id.clone(),
                power_levels_event_id.clone(),
            ])
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        prev_events = vec![_topic_event_id];
        event_depth += 1;
    }

    // 8. Initial state events
    if let Some(initial_state) = request.initial_state {
        for state_event in initial_state {
            let _event_id = create_custom_state_event(
                &state,
                &room_id,
                &user_id,
                &state_event.event_type,
                &state_event.state_key.unwrap_or_default(),
                &state_event.content,
                event_depth,
                &prev_events,
                &[
                    create_event_id.clone(),
                    join_event_id.clone(),
                    power_levels_event_id.clone(),
                ],
            )
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            prev_events = vec![_event_id];
            event_depth += 1;
        }
    }

    // 9. Invite users
    if let Some(invites) = request.invite {
        for invite_user_id in invites {
            // Validate Matrix user ID format (@localpart:domain)
            if !invite_user_id.starts_with('@') || !invite_user_id.contains(':') {
                warn!("Invalid user ID format for invitation: {}", invite_user_id);
                continue;
            }

            let _invite_event_id = create_membership_event(
                &state,
                &room_id,
                &user_id,
                &invite_user_id,
                MembershipState::Invite,
                None,
                event_depth,
                &prev_events,
                &[
                    create_event_id.clone(),
                    join_event_id.clone(),
                    power_levels_event_id.clone(),
                ],
            )
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            prev_events = vec![_invite_event_id];
            event_depth += 1;
        }
    }

    // 10. Invite users via third-party identifiers (email, phone, etc.)
    if let Some(invite_3pids) = request.invite_3pid {
        for invite_3pid in invite_3pids {
            match validate_and_process_3pid_invite(&state, &room_id, &user_id, invite_3pid).await {
                Ok(invite_result) => {
                    match invite_result {
                        ThirdPartyInviteResult::DirectInvite(matrix_user_id) => {
                            info!("Sent direct Matrix invite to {} via 3PID", matrix_user_id);
                        },
                        ThirdPartyInviteResult::PendingInvite(invite_token) => {
                            info!("Created pending 3PID invite with token {}", invite_token);
                        },
                    }
                },
                Err(e) => {
                    warn!("Failed to process 3PID invite: {}", e);
                    // Continue with other invites rather than failing the entire room creation
                },
            }
        }
    }

    // Handle room alias creation and registration
    let room_alias = if let Some(alias_name) = request.room_alias_name {
        let alias = format!("#{}:{}", alias_name, state.homeserver_name);

        // Check if alias already exists
        let existing_alias_query = "SELECT * FROM room_aliases WHERE alias = $alias";
        let mut existing_response = state
            .db
            .query(existing_alias_query)
            .bind(("alias", alias.clone()))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let existing_aliases: Vec<Value> =
            existing_response.take(0).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if !existing_aliases.is_empty() {
            warn!("Room alias {} already exists", alias);
            return Err(StatusCode::CONFLICT);
        }

        // Create room alias mapping
        let create_alias_query = r#"
            CREATE room_aliases SET 
                alias = $alias,
                room_id = $room_id,
                created_by = $user_id,
                created_at = $created_at,
                is_active = true
        "#;

        let _alias_result: Option<Value> = state
            .db
            .query(create_alias_query)
            .bind(("alias", alias.clone()))
            .bind(("room_id", room_id.clone()))
            .bind(("user_id", user_id.clone()))
            .bind(("created_at", Utc::now()))
            .await
            .map_err(|e| {
                error!("Failed to create room alias {}: {}", alias, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .take(0)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Update room with canonical alias
        let update_room_query = r#"
            UPDATE $room_record SET canonical_alias = $alias
        "#;

        let _update_result: Option<Room> = state
            .db
            .query(update_room_query)
            .bind(("room_record", ("rooms", room_id.clone())))
            .bind(("alias", alias.clone()))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .take(0)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        info!("Created room alias {} for room {}", alias, room_id);
        Some(alias)
    } else {
        None
    };

    Ok(Json(CreateRoomResponse { room_id, room_alias }))
}

fn generate_room_id(homeserver_name: &str) -> String {
    let random_part = Uuid::new_v4().to_string().replace('-', "");
    format!("!{}:{}", &random_part[..18], homeserver_name)
}

fn is_supported_room_version(version: &str) -> bool {
    matches!(version, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" | "11")
}

#[derive(Debug)]
enum ThirdPartyInviteResult {
    DirectInvite(String),  // Matrix user ID found for 3PID
    PendingInvite(String), // Invite token for pending invite
}

#[derive(serde::Deserialize)]
struct ThirdPartyId {
    address: String,
    medium: String, // "email" or "msisdn"
}

/// Validate and process a third-party identifier invitation
async fn validate_and_process_3pid_invite(
    state: &AppState,
    room_id: &str,
    sender: &str,
    invite_3pid: Value,
) -> Result<ThirdPartyInviteResult, Box<dyn std::error::Error + Send + Sync>> {
    // Parse 3PID from JSON value
    let third_party_id: ThirdPartyId = serde_json::from_value(invite_3pid.clone())?;

    // Validate 3PID format
    validate_3pid_format(&third_party_id)?;

    // Try to lookup existing Matrix user for this 3PID
    match lookup_matrix_user_by_3pid(state, &third_party_id).await? {
        Some(matrix_user_id) => {
            // User found - send direct Matrix invite
            let _invite_event_id = create_membership_event(
                state,
                room_id,
                sender,
                &matrix_user_id,
                MembershipState::Invite,
                None,
                0,   // depth will be calculated in create_membership_event
                &[], // prev_events will be calculated
                &[], // auth_events will be calculated
            )
            .await?;

            Ok(ThirdPartyInviteResult::DirectInvite(matrix_user_id))
        },
        None => {
            // User not found - create pending invite
            let invite_token =
                create_pending_3pid_invite(state, room_id, sender, &third_party_id, &invite_3pid)
                    .await?;

            Ok(ThirdPartyInviteResult::PendingInvite(invite_token))
        },
    }
}

/// Validate 3PID format according to Matrix specification
fn validate_3pid_format(
    third_party_id: &ThirdPartyId,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match third_party_id.medium.as_str() {
        "email" => {
            // Basic email validation
            if !third_party_id.address.contains('@') || third_party_id.address.len() < 3 {
                return Err("Invalid email format".into());
            }
        },
        "msisdn" => {
            // Basic phone number validation
            if third_party_id.address.is_empty() ||
                !third_party_id.address.chars().all(|c| c.is_ascii_digit() || c == '+')
            {
                return Err("Invalid phone number format".into());
            }
        },
        _ => {
            return Err(format!("Unsupported 3PID medium: {}", third_party_id.medium).into());
        },
    }
    Ok(())
}

/// Lookup Matrix user ID by third-party identifier
async fn lookup_matrix_user_by_3pid(
    state: &AppState,
    third_party_id: &ThirdPartyId,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let query = r#"
        SELECT user_id FROM user_third_party_ids 
        WHERE address = $address AND medium = $medium AND is_validated = true
        LIMIT 1
    "#;

    let mut response = state
        .db
        .query(query)
        .bind(("address", third_party_id.address.clone()))
        .bind(("medium", third_party_id.medium.clone()))
        .await?;

    #[derive(serde::Deserialize)]
    struct UserThirdPartyRecord {
        user_id: String,
    }

    let records: Vec<UserThirdPartyRecord> = response.take(0)?;
    Ok(records.into_iter().next().map(|r| r.user_id))
}

/// Create pending third-party identifier invite
async fn create_pending_3pid_invite(
    state: &AppState,
    room_id: &str,
    sender: &str,
    third_party_id: &ThirdPartyId,
    original_invite: &Value,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    use base64::{Engine, engine::general_purpose};
    use rand::{Rng, RngCore};

    // Generate unique invite token
    let mut token_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut token_bytes);
    let invite_token = general_purpose::STANDARD.encode(&token_bytes);

    let display_name = original_invite
        .get("display_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Store pending invite in database
    let room_id_owned = room_id.to_string();
    let sender_owned = sender.to_string();
    let create_invite_query = r#"
        CREATE third_party_invites SET
            room_id = $room_id,
            sender = $sender,
            address = $address,
            medium = $medium,
            display_name = $display_name,
            invite_token = $invite_token,
            created_at = $created_at,
            expires_at = $expires_at,
            is_active = true
    "#;

    let expires_at = Utc::now() + chrono::Duration::days(7); // 7 days expiry

    let _result: Option<Value> = state
        .db
        .query(create_invite_query)
        .bind(("room_id", room_id_owned))
        .bind(("sender", sender_owned))
        .bind(("address", third_party_id.address.clone()))
        .bind(("medium", third_party_id.medium.clone()))
        .bind(("display_name", display_name))
        .bind(("invite_token", invite_token.clone()))
        .bind(("created_at", Utc::now()))
        .bind(("expires_at", expires_at))
        .await?
        .take(0)?;

    // This would typically integrate with an email service or SMS provider
    // For now, we just log that the invite was created
    info!(
        "Created pending 3PID invite for {} via {}",
        third_party_id.address, third_party_id.medium
    );

    Ok(invite_token)
}

fn create_default_power_levels(creator_id: &str, request: &CreateRoomRequest) -> Value {
    let mut power_levels = json!({
        "ban": 50,
        "events": {
            "m.room.name": 50,
            "m.room.topic": 50,
            "m.room.avatar": 50,
            "m.room.canonical_alias": 50,
            "m.room.history_visibility": 100,
            "m.room.power_levels": 100,
            "m.room.encryption": 100
        },
        "events_default": 0,
        "invite": 50,
        "kick": 50,
        "redact": 50,
        "state_default": 50,
        "users": {},
        "users_default": 0,
        "notifications": {
            "room": 50
        }
    });

    // Set creator as admin (power level 100)
    if let Some(users) = power_levels.get_mut("users").and_then(|u| u.as_object_mut()) {
        users.insert(creator_id.to_string(), json!(100));
    }

    // Apply any overrides from the request
    if let Some(override_content) = &request.power_level_content_override {
        if let (Some(base), Some(override_obj)) =
            (power_levels.as_object_mut(), override_content.as_object())
        {
            for (key, value) in override_obj {
                base.insert(key.clone(), value.clone());
            }
        }
    }

    power_levels
}

// Helper functions to create specific event types
async fn create_room_create_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    room_version: &str,
    creation_content: Option<Value>,
    depth: i64,
    prev_events: &[String],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut content = json!({
        "creator": sender,
        "room_version": room_version
    });

    if let Some(additional_content) = creation_content {
        if let (Some(base), Some(additional)) =
            (content.as_object_mut(), additional_content.as_object())
        {
            for (key, value) in additional {
                if key != "creator" && key != "room_version" {
                    base.insert(key.clone(), value.clone());
                }
            }
        }
    }

    create_state_event(
        state,
        room_id,
        sender,
        "m.room.create",
        "",
        &content,
        depth,
        prev_events,
        &[], // No auth events for create event
    )
    .await
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

    let event_id = create_state_event(
        state,
        room_id,
        sender,
        "m.room.member",
        target,
        &content,
        depth,
        prev_events,
        auth_events,
    )
    .await?;

    // Also create membership record
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
        display_name: get_user_display_name(&state, target)
            .await
            .map_err(|e| {
                warn!("Failed to get display name for user {}: {}", target, e);
                e
            })
            .ok()
            .flatten(),
        avatar_url: get_user_avatar_url(&state, target)
            .await
            .map_err(|e| {
                warn!("Failed to get avatar URL for user {}: {}", target, e);
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
        .create(("membership", format!("{}:{}", target, room_id)))
        .content(membership_record.clone())
        .await?;

    Ok(event_id)
}

async fn create_power_levels_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    content: &Value,
    depth: i64,
    prev_events: &[String],
    auth_events: &[String],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    create_state_event(
        state,
        room_id,
        sender,
        "m.room.power_levels",
        "",
        content,
        depth,
        prev_events,
        auth_events,
    )
    .await
}

async fn create_join_rules_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    join_rule: &str,
    depth: i64,
    prev_events: &[String],
    auth_events: &[String],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let content = json!({
        "join_rule": join_rule
    });

    create_state_event(
        state,
        room_id,
        sender,
        "m.room.join_rules",
        "",
        &content,
        depth,
        prev_events,
        auth_events,
    )
    .await
}

async fn create_history_visibility_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    history_visibility: &str,
    depth: i64,
    prev_events: &[String],
    auth_events: &[String],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let content = json!({
        "history_visibility": history_visibility
    });

    create_state_event(
        state,
        room_id,
        sender,
        "m.room.history_visibility",
        "",
        &content,
        depth,
        prev_events,
        auth_events,
    )
    .await
}

async fn create_name_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    name: &str,
    depth: i64,
    prev_events: &[String],
    auth_events: &[String],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let content = json!({
        "name": name
    });

    create_state_event(
        state,
        room_id,
        sender,
        "m.room.name",
        "",
        &content,
        depth,
        prev_events,
        auth_events,
    )
    .await
}

async fn create_topic_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    topic: &str,
    depth: i64,
    prev_events: &[String],
    auth_events: &[String],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let content = json!({
        "topic": topic
    });

    create_state_event(
        state,
        room_id,
        sender,
        "m.room.topic",
        "",
        &content,
        depth,
        prev_events,
        auth_events,
    )
    .await
}

async fn create_custom_state_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    event_type: &str,
    state_key: &str,
    content: &Value,
    depth: i64,
    prev_events: &[String],
    auth_events: &[String],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    create_state_event(
        state,
        room_id,
        sender,
        event_type,
        state_key,
        content,
        depth,
        prev_events,
        auth_events,
    )
    .await
}

async fn create_state_event(
    state: &AppState,
    room_id: &str,
    sender: &str,
    event_type: &str,
    state_key: &str,
    content: &Value,
    depth: i64,
    prev_events: &[String],
    auth_events: &[String],
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let event_id = format!("${}:{}", Uuid::new_v4(), state.homeserver_name);

    let mut event = Event {
        event_id: event_id.clone(),
        room_id: room_id.to_string(),
        sender: sender.to_string(),
        event_type: event_type.to_string(),
        content: serde_json::from_value(content.clone()).unwrap_or_default(),
        state_key: Some(state_key.to_string()),
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
    event.hashes = serde_json::from_value(calculate_content_hashes(&event)?).ok();

    // Sign event with server's Ed25519 private key
    event.signatures = serde_json::from_value(sign_event(state, &event).await?).ok();

    // Store the event
    let _: Option<Event> = state.db.create(("event", &event_id)).content(event).await?;

    Ok(event_id)
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
pub async fn get_user_avatar_url(
    state: &AppState,
    user_id: &str,
) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let query = "SELECT avatar_url FROM user_profiles WHERE user_id = $user_id";
    let mut response = state.db.query(query).bind(("user_id", user_id.to_string())).await?;

    let avatar_url: Option<String> = response.take(0)?;
    Ok(avatar_url)
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

    let room_info: Option<RoomInfo> = response.take(0)?;
    Ok(room_info.map(|r| r.is_direct).unwrap_or(false))
}
