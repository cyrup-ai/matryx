use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_entity::types::{Membership, MembershipState};

#[derive(Deserialize)]
pub struct MembersQuery {
    /// The point in time (pagination token) to return members for in the room.
    /// This token can be obtained from a prev_batch token returned by a previous
    /// request to this endpoint, or from a start or end token returned by a
    /// /sync or /messages request.
    #[serde(rename = "at")]
    at: Option<String>,

    /// The kind of membership to filter for. Defaults to no filtering if
    /// unspecified. When specified alongside not_membership, the two
    /// parameters create an 'or' condition: either the membership IS one
    /// of the specified values, OR the membership IS NOT one of the values
    /// specified by not_membership.
    membership: Option<String>,

    /// The kind of membership to exclude from the results. Defaults to no
    /// filtering if unspecified.
    not_membership: Option<String>,
}

#[derive(Serialize)]
pub struct MemberEvent {
    /// The event content
    content: Value,

    /// The event ID
    event_id: String,

    /// The timestamp for when the event was sent
    origin_server_ts: i64,

    /// The user ID of the user who sent this event
    sender: String,

    /// The user ID for whom this membership event applies to
    state_key: String,

    /// The event type - always "m.room.member" for membership events
    #[serde(rename = "type")]
    event_type: String,

    /// The event's unsigned data
    #[serde(skip_serializing_if = "Option::is_none")]
    unsigned: Option<Value>,
}

#[derive(Serialize)]
pub struct MembersResponse {
    /// A list of the most recent membership events for each user
    chunk: Vec<MemberEvent>,
}

/// GET /_matrix/client/v3/rooms/{roomId}/members
///
/// Get the list of members for a room.
///
/// This endpoint returns the most recent membership event for each user who has
/// ever been a member of the room. Users who have left or been kicked/banned
/// may still appear in this list, depending on the filtering parameters.
pub async fn get(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<MembersQuery>,
) -> Result<Json<MembersResponse>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers).map_err(|e| {
        warn!("Room members request failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Room members request failed - access token expired");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            // Server-to-server requests are allowed for federation
            "server".to_string()
        },
        MatrixAuth::Anonymous => {
            warn!("Room members request failed - anonymous authentication not allowed");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing room members request for room: {} by user: {}", room_id, user_id);

    // Validate room ID format
    if !room_id.starts_with('!') {
        warn!("Room members request failed - invalid room ID format: {}", room_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if user can access room members (must be a member of the room)
    if user_id != "server" {
        let user_membership = get_user_membership(&state, &room_id, &user_id).await
            .map_err(|_| {
                warn!("Room members request failed - could not determine membership for user {} in room {}", 
                      user_id, room_id);
                StatusCode::FORBIDDEN
            })?;

        // User must be joined or invited to see member list
        match user_membership.membership {
            MembershipState::Join | MembershipState::Invite => {
                // User can access member list
            },
            _ => {
                warn!(
                    "Room members request failed - user {} not authorized to view members of room {}",
                    user_id, room_id
                );
                return Err(StatusCode::FORBIDDEN);
            },
        }
    }

    // Parse membership filtering
    let membership_filter = query.membership.as_deref().map(parse_membership_state);
    let not_membership_filter = query.not_membership.as_deref().map(parse_membership_state);

    // Get room members from database
    let members = get_room_members(&state, &room_id, membership_filter, not_membership_filter)
        .await
        .map_err(|e| {
            error!("Failed to get room members for room {}: {}", room_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    info!("Successfully retrieved {} members for room {}", members.len(), room_id);

    Ok(Json(MembersResponse { chunk: members }))
}
/// Get current membership for a user in a room
async fn get_user_membership(
    state: &AppState,
    room_id: &str,
    user_id: &str,
) -> Result<Membership, Box<dyn std::error::Error + Send + Sync>> {
    let membership_id = format!("{}:{}", user_id, room_id);
    let membership: Option<Membership> = state.db.select(("membership", membership_id)).await?;

    membership.ok_or_else(|| "Membership not found".into())
}

/// Get all members of a room with optional filtering
async fn get_room_members(
    state: &AppState,
    room_id: &str,
    membership_filter: Option<MembershipState>,
    not_membership_filter: Option<MembershipState>,
) -> Result<Vec<MemberEvent>, Box<dyn std::error::Error + Send + Sync>> {
    // Build query with membership filtering
    let mut query = "
        SELECT 
            m.user_id,
            m.membership, 
            m.display_name,
            m.avatar_url,
            m.updated_at,
            e.event_id,
            e.content,
            e.origin_server_ts,
            e.sender,
            e.unsigned
        FROM membership m
        LEFT JOIN event e ON (
            e.room_id = m.room_id 
            AND e.state_key = m.user_id 
            AND e.event_type = 'm.room.member'
        )
        WHERE m.room_id = $room_id
    "
    .to_string();

    // Add membership filtering
    if let Some(membership) = membership_filter {
        let membership_str = membership_to_string(&membership);
        query.push_str(&format!(" AND m.membership = '{}'", membership_str));
    }

    if let Some(not_membership) = not_membership_filter {
        let not_membership_str = membership_to_string(&not_membership);
        query.push_str(&format!(" AND m.membership != '{}'", not_membership_str));
    }

    query.push_str(" ORDER BY m.updated_at DESC");

    debug!("Executing room members query: {}", query);

    let mut response = state.db.query(&query).bind(("room_id", room_id.to_string())).await?;

    #[derive(serde::Deserialize)]
    struct MemberRecord {
        user_id: String,
        membership: String,
        display_name: Option<String>,
        avatar_url: Option<String>,
        updated_at: chrono::DateTime<chrono::Utc>,
        event_id: Option<String>,
        content: Option<Value>,
        origin_server_ts: Option<chrono::DateTime<chrono::Utc>>,
        sender: Option<String>,
        unsigned: Option<Value>,
    }

    let member_records: Vec<MemberRecord> = response.take(0)?;

    let mut member_events = Vec::new();

    for record in member_records {
        // If we have the original membership event, use it
        if let (Some(event_id), Some(content), Some(origin_server_ts), Some(sender)) =
            (record.event_id, record.content, record.origin_server_ts, record.sender)
        {
            member_events.push(MemberEvent {
                content,
                event_id,
                origin_server_ts: origin_server_ts.timestamp_millis(),
                sender,
                state_key: record.user_id,
                event_type: "m.room.member".to_string(),
                unsigned: record.unsigned,
            });
        } else {
            // Create synthetic membership event from membership record
            let mut content_obj = HashMap::new();
            content_obj.insert("membership".to_string(), json!(record.membership));

            if let Some(display_name) = record.display_name {
                content_obj.insert("displayname".to_string(), json!(display_name));
            }

            if let Some(avatar_url) = record.avatar_url {
                content_obj.insert("avatar_url".to_string(), json!(avatar_url));
            }

            member_events.push(MemberEvent {
                content: json!(content_obj),
                event_id: format!(
                    "$synthetic:{}:{}:{}",
                    record.user_id,
                    room_id,
                    record.updated_at.timestamp_millis()
                ),
                origin_server_ts: record.updated_at.timestamp_millis(),
                sender: record.user_id.clone(),
                state_key: record.user_id,
                event_type: "m.room.member".to_string(),
                unsigned: None,
            });
        }
    }

    debug!("Retrieved {} member events for room {}", member_events.len(), room_id);
    Ok(member_events)
}

/// Parse membership state string
fn parse_membership_state(membership_str: &str) -> MembershipState {
    match membership_str {
        "join" => MembershipState::Join,
        "leave" => MembershipState::Leave,
        "invite" => MembershipState::Invite,
        "ban" => MembershipState::Ban,
        "knock" => MembershipState::Knock,
        _ => MembershipState::Leave, // Default fallback
    }
}

/// Convert membership state to string
fn membership_to_string(membership: &MembershipState) -> &'static str {
    match membership {
        MembershipState::Join => "join",
        MembershipState::Leave => "leave",
        MembershipState::Invite => "invite",
        MembershipState::Ban => "ban",
        MembershipState::Knock => "knock",
    }
}
