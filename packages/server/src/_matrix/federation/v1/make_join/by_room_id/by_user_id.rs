use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};

use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use crate::federation::membership_federation::validate_federation_join_allowed;
use matryx_entity::types::{MembershipState, Room};
use matryx_surrealdb::repository::{EventRepository, MembershipRepository, RoomRepository};

/// Query parameters for make_join request
#[derive(Debug, Deserialize)]
pub struct MakeJoinQuery {
    /// The room versions the sending server has support for
    ver: Option<Vec<String>>,
}

/// Matrix X-Matrix authentication header parsed structure
#[derive(Debug, Clone)]
struct XMatrixAuth {
    origin: String,
    key_id: String,
    signature: String,
}

/// Parse X-Matrix authentication header
fn parse_x_matrix_auth(headers: &HeaderMap) -> Result<XMatrixAuth, StatusCode> {
    let auth_header = headers
        .get("authorization")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if !auth_header.starts_with("X-Matrix ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let auth_params = &auth_header[9..]; // Skip "X-Matrix "

    let mut origin = None;
    let mut key = None;
    let mut signature = None;

    // Parse comma-separated key=value pairs
    for param in auth_params.split(',') {
        let param = param.trim();

        if let Some((key_name, value)) = param.split_once('=') {
            match key_name.trim() {
                "origin" => {
                    origin = Some(value.trim().to_string());
                },
                "key" => {
                    // Extract key_id from "ed25519:key_id" format
                    let key_value = value.trim().trim_matches('"');
                    if let Some(key_id) = key_value.strip_prefix("ed25519:") {
                        key = Some(key_id.to_string());
                    } else {
                        return Err(StatusCode::BAD_REQUEST);
                    }
                },
                "sig" => {
                    signature = Some(value.trim().trim_matches('"').to_string());
                },
                _ => {
                    // Unknown parameter, ignore for forward compatibility
                },
            }
        }
    }

    let origin = origin.ok_or(StatusCode::BAD_REQUEST)?;
    let key_id = key.ok_or(StatusCode::BAD_REQUEST)?;
    let signature = signature.ok_or(StatusCode::BAD_REQUEST)?;

    Ok(XMatrixAuth { origin, key_id, signature })
}

/// GET /_matrix/federation/v1/make_join/{roomId}/{userId}
///
/// Asks the receiving server to return information that the sending server
/// will need to prepare a join event to get into the room.
pub async fn get(
    State(state): State<AppState>,
    Path((room_id, user_id)): Path<(String, String)>,
    Query(query): Query<MakeJoinQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
    })?;

    debug!(
        "make_join request - origin: {}, room: {}, user: {}",
        x_matrix_auth.origin, room_id, user_id
    );

    // Validate server signature
    let request_body = format!("room_id={}&user_id={}", room_id, user_id);
    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "GET",
            "/make_join",
            request_body.as_bytes(),
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Validate that the user belongs to the requesting server
    let user_domain = user_id.split(':').nth(1).unwrap_or("");
    if user_domain != x_matrix_auth.origin {
        warn!("User {} doesn't belong to origin server {}", user_id, x_matrix_auth.origin);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get room information from database
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let event_repo = Arc::new(EventRepository::new(state.db.clone()));

    let room = room_repo
        .get_by_id(&room_id)
        .await
        .map_err(|e| {
            error!("Failed to query room {}: {}", room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("Room {} not found", room_id);
            StatusCode::NOT_FOUND
        })?;

    // Validate room allows federation joins from this origin server
    if !validate_federation_join_allowed(&room, &x_matrix_auth.origin) {
        warn!(
            "Federation join denied for server {} in room {} - federation restrictions apply",
            x_matrix_auth.origin, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    info!(
        "Room validation passed for make_join request in room {} from server {}",
        room_id, x_matrix_auth.origin
    );

    // Check room version compatibility
    let supported_versions = query.ver.unwrap_or_else(|| vec!["1".to_string()]);
    let room_version = room.room_version.clone();

    if !supported_versions.contains(&room_version) {
        warn!(
            "Room version {} not supported by joining server. Supported: {:?}",
            room_version, supported_versions
        );
        return Ok(Json(json!({
            "errcode": "M_INCOMPATIBLE_ROOM_VERSION",
            "error": format!("Room version {} not supported", room_version),
            "room_version": room_version
        })));
    }

    // Check if user is already a member of the room
    if let Ok(Some(existing_membership)) =
        membership_repo.get_by_room_user(&room_id, &user_id).await
    {
        match existing_membership.membership {
            MembershipState::Join => {
                warn!("User {} already joined room {}", user_id, room_id);
                return Err(StatusCode::BAD_REQUEST);
            },
            MembershipState::Ban => {
                warn!("User {} is banned from room {}", user_id, room_id);
                return Err(StatusCode::FORBIDDEN);
            },
            _ => {
                // User has other membership status, proceed with join
            },
        }
    }

    // Check room join rules and permissions
    let can_join = check_join_authorization(&state, &room, &user_id, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to check join authorization: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !can_join.allowed {
        warn!("User {} not authorized to join room {}: {}", user_id, room_id, can_join.reason);
        return match can_join.error_code.as_str() {
            "M_UNABLE_TO_AUTHORISE_JOIN" => {
                Ok(Json(json!({
                    "errcode": "M_UNABLE_TO_AUTHORISE_JOIN",
                    "error": can_join.reason
                })))
            },
            "M_UNABLE_TO_GRANT_JOIN" => {
                Ok(Json(json!({
                    "errcode": "M_UNABLE_TO_GRANT_JOIN",
                    "error": can_join.reason
                })))
            },
            _ => Err(StatusCode::FORBIDDEN),
        };
    }

    // Build the join event template
    let mut event_content = json!({
        "membership": "join"
    });

    // Add join_authorised_via_users_server if room is restricted and we have an authorizing user
    if let Some(ref authorizing_user) = can_join.authorizing_user {
        event_content["join_authorised_via_users_server"] = json!(authorizing_user);
    }

    let now = Utc::now().timestamp_millis();

    // Get auth_events required for Matrix federation compliance
    let auth_events = event_repo
        .get_auth_events_for_join(&room_id, &user_id)
        .await
        .map_err(|e| {
            error!("Failed to fetch auth_events for join in room {}: {}", room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Get prev_events for proper event DAG construction per Matrix specification
    let prev_events = event_repo
        .get_room_events(&room_id, Some(10))
        .await
        .map_err(|e| {
            error!("Failed to fetch prev_events for room {}: {}", room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    debug!(
        "Matrix join event preparation: auth_events={}, prev_events={} for user {} in room {}",
        auth_events.len(),
        prev_events.len(),
        user_id,
        room_id
    );

    let event_template = json!({
        "type": "m.room.member",
        "content": event_content,
        "state_key": user_id,
        "room_id": room_id,
        "sender": user_id,
        "origin": state.homeserver_name,
        "origin_server_ts": now,
        "auth_events": auth_events,
        "prev_events": prev_events
    });

    let response = json!({
        "event": event_template,
        "room_version": room_version
    });

    info!(
        "Created make_join template for user {} in room {} (version {})",
        user_id, room_id, room_version
    );

    Ok(Json(response))
}

/// Result of join authorization check
#[derive(Debug)]
struct JoinAuthResult {
    allowed: bool,
    reason: String,
    error_code: String,
    authorizing_user: Option<String>,
}

/// Check if a user is authorized to join a room based on join rules
async fn check_join_authorization(
    state: &AppState,
    room: &Room,
    user_id: &str,
    origin_server: &str,
) -> Result<JoinAuthResult, Box<dyn std::error::Error + Send + Sync>> {
    // Validate that the origin server matches the user's server domain
    let user_server = user_id.split(':').nth(1)
        .ok_or("Invalid user ID format")?;
    
    if user_server != origin_server {
        return Err(format!("Origin server mismatch: user {} not from server {}", user_id, origin_server).into());
    }

    // Get room's join rules from current state
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let join_rules = room_repo.get_room_join_rules_string(&room.room_id).await
        .map_err(|e| format!("Failed to get room join rules: {}", e))?;

    match join_rules.as_str() {
        "public" => {
            // Anyone can join public rooms
            Ok(JoinAuthResult {
                allowed: true,
                reason: "Public room".to_string(),
                error_code: String::new(),
                authorizing_user: None,
            })
        },
        "invite" => {
            // Must have invite to join invite-only rooms
            let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));

            match membership_repo.get_by_room_user(&room.room_id, user_id).await? {
                Some(membership) if membership.membership == MembershipState::Invite => {
                    Ok(JoinAuthResult {
                        allowed: true,
                        reason: "User has invite".to_string(),
                        error_code: String::new(),
                        authorizing_user: None,
                    })
                },
                _ => {
                    Ok(JoinAuthResult {
                        allowed: false,
                        reason: "Room requires invite to join".to_string(),
                        error_code: "M_FORBIDDEN".to_string(),
                        authorizing_user: None,
                    })
                },
            }
        },
        "knock" => {
            // Must knock before joining knock rooms
            Ok(JoinAuthResult {
                allowed: false,
                reason: "Room requires knocking before joining".to_string(),
                error_code: "M_FORBIDDEN".to_string(),
                authorizing_user: None,
            })
        },
        "restricted" => {
            // Check restricted room conditions
            check_restricted_room_conditions(state, room, user_id, origin_server).await
        },
        _ => {
            // Unknown join rule - default to forbidden
            Ok(JoinAuthResult {
                allowed: false,
                reason: format!("Unknown join rule: {}", join_rules),
                error_code: "M_FORBIDDEN".to_string(),
                authorizing_user: None,
            })
        },
    }
}



/// Check restricted room join conditions
async fn check_restricted_room_conditions(
    state: &AppState,
    room: &Room,
    user_id: &str,
    origin_server: &str,
) -> Result<JoinAuthResult, Box<dyn std::error::Error + Send + Sync>> {
    // Validate that the origin server matches the user's server domain
    let user_server = user_id.split(':').nth(1)
        .ok_or("Invalid user ID format")?;
    
    if user_server != origin_server {
        return Err(format!("Origin server mismatch: user {} not from server {}", user_id, origin_server).into());
    }

    // Get room's join rule content for allow conditions
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let join_rule_content = room_repo.get_room_join_rule_content(&room.room_id).await
        .map_err(|e| format!("Failed to get room join rule content: {}", e))?;

    let allow_conditions = join_rule_content
        .get("allow")
        .and_then(|v| v.as_array())
        .ok_or("No allow conditions in restricted room")?;

    // Check each allow condition
    for condition in allow_conditions {
        let condition_type = condition.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match condition_type {
            "m.room_membership" => {
                // Check if user is a member of a specified room
                let condition_room_id = condition
                    .get("room_id")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing room_id in room membership condition")?;

                if let Ok(authorizing_user) = check_room_membership_condition(
                    state,
                    user_id,
                    condition_room_id,
                    origin_server,
                )
                .await
                {
                    return Ok(JoinAuthResult {
                        allowed: true,
                        reason: format!("User is member of allowed room {}", condition_room_id),
                        error_code: String::new(),
                        authorizing_user: Some(authorizing_user),
                    });
                }
            },
            _ => {
                debug!("Unknown allow condition type: {}", condition_type);
            },
        }
    }

    // No conditions satisfied
    Ok(JoinAuthResult {
        allowed: false,
        reason: "Unable to satisfy restricted room conditions".to_string(),
        error_code: "M_UNABLE_TO_AUTHORISE_JOIN".to_string(),
        authorizing_user: None,
    })
}



/// Check if user satisfies room membership condition
async fn check_room_membership_condition(
    state: &AppState,
    user_id: &str,
    condition_room_id: &str,
    origin_server: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Validate that the origin server matches the user's server domain
    let user_server = user_id.split(':').nth(1)
        .ok_or("Invalid user ID format")?;
    
    if user_server != origin_server {
        return Err(format!("Origin server mismatch: user {} not from server {}", user_id, origin_server).into());
    }

    // Check if our server knows about the condition room
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
    let condition_room = room_repo
        .get_by_id(condition_room_id)
        .await?
        .ok_or("Condition room not known to this server")?;

    // Validate condition room properties for restricted join authorization
    if condition_room.room_version.is_empty() {
        return Err("Invalid condition room: missing room version".into());
    }
    
    debug!("Validating membership in condition room {} (version: {})", 
           condition_room_id, condition_room.room_version);

    // Check if the user is a member of the condition room
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    let membership = membership_repo.get_by_room_user(condition_room_id, user_id).await?;

    match membership {
        Some(m) if m.membership == MembershipState::Join => {
            // Find a user from our server who can authorize the join
            let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
            membership_repo.find_authorizing_user(condition_room_id, &state.homeserver_name).await
                .map_err(|e| format!("Failed to find authorizing user: {}", e))?
                .ok_or("No authorizing user found from our server".into())
        },
        _ => Err("User is not a member of the condition room".into()),
    }
}


