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
use matryx_entity::types::MembershipState;
use matryx_surrealdb::repository::{MembershipRepository, RoomRepository};

/// Matrix X-Matrix authentication header parsed structure
#[derive(Debug, Clone)]
struct XMatrixAuth {
    origin: String,
    key_id: String,
    signature: String,
}

/// Query parameters for make_knock endpoint
#[derive(Debug, Deserialize)]
pub struct MakeKnockQuery {
    pub ver: Vec<String>,
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

/// GET /_matrix/federation/v1/make_knock/{roomId}/{userId}
///
/// Asks the receiving server to return information that the sending server will need
/// to prepare a knock event for the room.
pub async fn get(
    State(state): State<AppState>,
    Path((room_id, user_id)): Path<(String, String)>,
    Query(query): Query<MakeKnockQuery>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).inspect_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
    })?;

    debug!(
        "make_knock request - origin: {}, room: {}, user: {}",
        x_matrix_auth.origin, room_id, user_id
    );

    // Validate server signature
    let request_uri = format!("/make_knock/{}/{}", room_id, user_id);
    let query_string = format!("ver={}", query.ver.join("&ver="));
    let full_uri = format!("{}?{}", request_uri, query_string);

    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "GET",
            &full_uri,
            &[],
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

    // Validate the room exists and we know about it
    let room_repo = Arc::new(RoomRepository::new(state.db.clone()));
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

    // Check room version compatibility
    if !query.ver.contains(&room.room_version) {
        warn!(
            "Room version {} not supported by requesting server. Supported: {:?}",
            room.room_version, query.ver
        );
        return Ok(Json(json!({
            "errcode": "M_INCOMPATIBLE_ROOM_VERSION",
            "error": format!("Your homeserver does not support the features required to knock on this room"),
            "room_version": room.room_version
        })));
    }

    // Check if room allows knocking
    let join_rules_valid = room_repo.check_room_allows_knocking(&room_id).await.map_err(|e| {
        error!("Failed to check room join rules: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !join_rules_valid {
        warn!("Room {} does not allow knocking", room_id);
        return Ok(Json(json!({
            "errcode": "M_FORBIDDEN",
            "error": "You are not permitted to knock on this room"
        })));
    }

    // Check if user is already in the room or banned
    let membership_repo = Arc::new(MembershipRepository::new(state.db.clone()));
    if let Ok(Some(existing_membership)) =
        membership_repo.get_by_room_user(&room_id, &user_id).await
    {
        match existing_membership.membership {
            MembershipState::Join => {
                warn!("User {} is already joined to room {}", user_id, room_id);
                return Ok(Json(json!({
                    "errcode": "M_FORBIDDEN",
                    "error": "You are already in the room"
                })));
            },
            MembershipState::Ban => {
                warn!("User {} is banned from room {}", user_id, room_id);
                return Ok(Json(json!({
                    "errcode": "M_FORBIDDEN",
                    "error": "You are banned from the room"
                })));
            },
            MembershipState::Knock => {
                warn!("User {} is already knocking on room {}", user_id, room_id);
                return Ok(Json(json!({
                    "errcode": "M_FORBIDDEN",
                    "error": "You are already knocking on this room"
                })));
            },
            MembershipState::Invite => {
                warn!("User {} is already invited to room {}", user_id, room_id);
                return Ok(Json(json!({
                    "errcode": "M_FORBIDDEN",
                    "error": "You are already invited to this room"
                })));
            },
            _ => {
                // User has other membership status (e.g., left), can proceed with knock
            },
        }
    }

    // Check server ACLs
    let server_allowed = room_repo.check_server_acls(&room_id, &x_matrix_auth.origin)
        .await
        .map_err(|e| {
            error!("Failed to check server ACLs: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if !server_allowed {
        warn!("Server {} is denied by room ACLs", x_matrix_auth.origin);
        return Ok(Json(json!({
            "errcode": "M_FORBIDDEN",
            "error": "Your server is not permitted to knock on this room"
        })));
    }

    // Generate knock event template
    let now = Utc::now();
    let origin_server_ts = now.timestamp_millis();

    let event_template = json!({
        "type": "m.room.member",
        "content": {
            "membership": "knock"
        },
        "origin": state.homeserver_name,
        "origin_server_ts": origin_server_ts,
        "room_id": room_id,
        "sender": user_id,
        "state_key": user_id
    });

    let response = json!({
        "event": event_template,
        "room_version": room.room_version
    });

    info!("Successfully generated knock template for user {} in room {}", user_id, room_id);

    Ok(Json(response))
}


