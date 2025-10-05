use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::federation::membership_federation::validate_federation_leave_allowed;
use crate::state::AppState;
use matryx_entity::types::MembershipState;
use matryx_surrealdb::repository::{MembershipRepository, RoomRepository};

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

/// GET /_matrix/federation/v1/make_leave/{roomId}/{userId}
///
/// Asks the receiving server to return information that the sending server
/// will need to prepare a leave event to get out of the room.
pub async fn get(
    State(state): State<AppState>,
    Path((room_id, user_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
    })?;

    debug!(
        "make_leave request - origin: {}, room: {}, user: {}",
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
            "/make_leave",
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

    // Validate room allows leave events from this federation server
    if !validate_federation_leave_allowed(&room, &x_matrix_auth.origin) {
        warn!(
            "Federation leave denied for server {} in room {} - origin restrictions apply",
            x_matrix_auth.origin, room_id
        );
        return Err(StatusCode::FORBIDDEN);
    }

    info!(
        "Room validation passed for make_leave request in room {} from server {}",
        room_id, x_matrix_auth.origin
    );

    // Check if user is in the room (must be joined or invited to leave)
    let existing_membership =
        membership_repo.get_by_room_user(&room_id, &user_id).await.map_err(|e| {
            error!("Failed to query membership for user {} in room {}: {}", user_id, room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match existing_membership {
        Some(membership) => {
            match membership.membership {
                MembershipState::Join | MembershipState::Invite => {
                    // User can leave from joined or invited state
                },
                MembershipState::Leave => {
                    warn!("User {} has already left room {}", user_id, room_id);
                    return Ok(Json(json!({
                        "errcode": "M_FORBIDDEN",
                        "error": "User has already left the room"
                    })));
                },
                MembershipState::Ban => {
                    warn!("User {} is banned from room {} and cannot leave", user_id, room_id);
                    return Ok(Json(json!({
                        "errcode": "M_FORBIDDEN",
                        "error": "User is banned and cannot leave"
                    })));
                },
                MembershipState::Knock => {
                    // User can leave from knock state (withdraw knock)
                },
            }
        },
        None => {
            warn!("User {} is not in room {}", user_id, room_id);
            return Ok(Json(json!({
                "errcode": "M_FORBIDDEN",
                "error": "User is not in the room"
            })));
        },
    }

    // Build the leave event template
    let now = Utc::now().timestamp_millis();

    let event_template = json!({
        "type": "m.room.member",
        "content": {
            "membership": "leave"
        },
        "state_key": user_id,
        "room_id": room_id,
        "sender": user_id,
        "origin": state.homeserver_name,
        "origin_server_ts": now
    });

    let response = json!({
        "event": event_template,
        "room_version": room.room_version
    });

    info!(
        "Created make_leave template for user {} in room {} (version {})",
        user_id, room_id, room.room_version
    );

    Ok(Json(response))
}
