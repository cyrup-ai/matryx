use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info, warn};
use uuid;

use crate::{
    AppState,
    auth::{MatrixAuth, extract_matrix_auth},
};
use matryx_entity::types::MembershipState;

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
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
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

    // Convert query parameters to appropriate types
    let membership_filter = query.membership.map(MembershipState::from);
    let not_membership_filter = query.not_membership.map(MembershipState::from);

    // Use RoomOperationsService to get room members with Matrix spec compliance
    match state
        .room_operations
        .get_room_member_list(
            &room_id,
            query.at.as_deref(),
            membership_filter,
            not_membership_filter,
        )
        .await
    {
        Ok(members) => {
            info!("Successfully retrieved {} members for room {}", members.len(), room_id);

            // Convert RoomMember to MemberEvent per Matrix specification
            let member_events: Vec<MemberEvent> = members
                .into_iter()
                .map(|member| {
                    let content = serde_json::json!({
                        "membership": member.membership.to_string().to_lowercase(),
                        "displayname": member.display_name,
                        "avatar_url": member.avatar_url,
                        "reason": member.reason
                    });

                    MemberEvent {
                        content,
                        event_id: format!("${}:{}", uuid::Uuid::new_v4(), state.homeserver_name),
                        origin_server_ts: member.updated_at.timestamp_millis(),
                        sender: member.invited_by.unwrap_or_else(|| member.user_id.clone()),
                        state_key: member.user_id,
                        event_type: "m.room.member".to_string(),
                        unsigned: None,
                    }
                })
                .collect();

            Ok(Json(MembersResponse { chunk: member_events }))
        },
        Err(e) => {
            error!("Failed to get room members for room {}: {}", room_id, e);
            match e {
                matryx_surrealdb::repository::error::RepositoryError::NotFound { .. } => {
                    Err(StatusCode::NOT_FOUND)
                },
                matryx_surrealdb::repository::error::RepositoryError::Unauthorized { .. } => {
                    Err(StatusCode::FORBIDDEN)
                },
                matryx_surrealdb::repository::error::RepositoryError::Validation { .. } => {
                    Err(StatusCode::BAD_REQUEST)
                },
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        },
    }
}
