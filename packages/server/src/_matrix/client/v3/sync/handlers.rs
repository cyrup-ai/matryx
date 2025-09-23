use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::ACCEPT},
    response::IntoResponse,
};
use chrono::Utc;
use std::collections::HashMap;
use tracing::warn;

use crate::_matrix::client::v3::sync::data::{
    apply_account_data_filter,
    apply_presence_filter,
    convert_events_to_matrix_format,
    get_default_timeline_events,
    get_invited_member_count,
    get_joined_member_count,
    get_room_ephemeral_events,
    get_room_heroes,
    get_room_state_events,
    get_user_account_data,
    get_user_memberships,
    get_user_presence_events,
    set_user_presence,
};
use crate::_matrix::client::v3::sync::filters::{
    apply_event_fields_filter,
    apply_lazy_loading_filter,
    apply_room_filter,
    get_filtered_timeline_events,
    resolve_filter,
};
use crate::_matrix::client::v3::sync::streaming::get_sse_stream;
use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use matryx_entity::types::{
    AccountDataResponse,
    DeviceListsResponse,
    EphemeralResponse,
    InvitedRoomResponse,
    JoinedRoomResponse,
    LeftRoomResponse,
    MatrixFilter,
    Membership,
    MembershipState,
    PresenceResponse,
    Room,
    RoomSummary,
    RoomsResponse,
    StateResponse,
    SyncQuery,
    SyncResponse,
    TimelineResponse,
    ToDeviceResponse,
    UnreadNotifications,
};

/// GET /_matrix/client/v3/sync
///
/// Matrix sync endpoint with support for both traditional HTTP long-polling
/// and Server-Sent Events (SSE) streaming based on client Accept headers.
///
/// - Standard sync: Returns JSON response with current state
/// - SSE streaming: Returns text/event-stream with real-time updates
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    auth: AuthenticatedUser,
    Query(query): Query<SyncQuery>,
) -> Result<impl IntoResponse, StatusCode> {
    // Check if client wants SSE streaming
    let wants_sse = headers
        .get(ACCEPT)
        .and_then(|h| h.to_str().ok())
        .map(|accept| accept.contains("text/event-stream"))
        .unwrap_or(false);

    if wants_sse {
        // Return SSE streaming response
        get_sse_stream(state, auth, query).await.map(IntoResponse::into_response)
    } else {
        // Return traditional JSON sync response
        get_json_sync(state, auth, query).await.map(IntoResponse::into_response)
    }
}

/// Traditional HTTP JSON sync implementation
pub async fn get_json_sync(
    state: AppState,
    auth: AuthenticatedUser,
    query: SyncQuery,
) -> Result<Json<SyncResponse>, StatusCode> {
    // Access authentication fields to ensure they're used
    let _auth_device_id = &auth.device_id;
    let _auth_access_token = &auth.access_token;

    let user_id = &auth.user_id;

    // Handle sync parameters
    let _since_token = query.since.as_deref();
    let filter_param = query.filter.as_deref();
    let _full_state = query.full_state.unwrap_or(false);
    let _timeout_ms = query.timeout.unwrap_or(30000); // Default 30 seconds

    // Process filter parameter
    let applied_filter = if let Some(filter_param) = filter_param {
        resolve_filter(&state, filter_param, user_id).await?
    } else {
        None
    };

    // Handle presence setting
    if let Some(presence) = &query.set_presence {
        set_user_presence(&state, &auth.user_id, presence).await.map_err(|e| {
            warn!("Failed to set user presence: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        tracing::debug!("Set user presence to: {}", presence);
    }

    // Generate batch token - in production this would be more sophisticated
    let next_batch = format!("s{}", Utc::now().timestamp_millis());

    // Get user's room memberships
    let memberships = get_user_memberships(&state, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Apply filter to memberships
    let filtered_memberships = if let Some(ref filter) = applied_filter {
        apply_room_filter(memberships, filter)
    } else {
        memberships
    };

    // Separate rooms by membership state
    let mut joined_rooms = HashMap::new();
    let mut invited_rooms = HashMap::new();
    let mut left_rooms = HashMap::new();

    for membership in filtered_memberships {
        match membership.membership {
            MembershipState::Join => {
                let room_response = build_joined_room_response(
                    &state,
                    &membership.room_id,
                    user_id,
                    &query,
                    applied_filter.as_ref(),
                )
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                joined_rooms.insert(membership.room_id, room_response);
            },
            MembershipState::Invite => {
                let room_response = build_invited_room_response(&state, &membership.room_id)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                invited_rooms.insert(membership.room_id, room_response);
            },
            MembershipState::Leave => {
                let room_response = build_left_room_response(&state, &membership.room_id, user_id)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                left_rooms.insert(membership.room_id, room_response);
            },
            _ => {}, // Skip ban, knock for now
        }
    }

    // Apply presence filtering
    let presence_events =
        if let Some(presence_filter) = applied_filter.as_ref().and_then(|f| f.presence.as_ref()) {
            let raw_presence = get_user_presence_events(&state, user_id)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            apply_presence_filter(raw_presence, presence_filter)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        } else {
            get_user_presence_events(&state, user_id)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        };

    // Apply account data filtering
    let account_data_events = if let Some(account_filter) =
        applied_filter.as_ref().and_then(|f| f.account_data.as_ref())
    {
        let raw_account_data = get_user_account_data(&state, user_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        apply_account_data_filter(raw_account_data, account_filter)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        get_user_account_data(&state, user_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

    let response = SyncResponse {
        next_batch,
        rooms: RoomsResponse {
            join: joined_rooms,
            invite: invited_rooms,
            leave: left_rooms,
        },
        presence: PresenceResponse { events: presence_events },
        account_data: AccountDataResponse { events: account_data_events },
        to_device: ToDeviceResponse { events: Vec::new() },
        device_lists: DeviceListsResponse { changed: Vec::new(), left: Vec::new() },
        device_one_time_keys_count: HashMap::new(),
    };

    Ok(Json(response))
}

pub async fn build_joined_room_response(
    state: &AppState,
    room_id: &str,
    user_id: &str,
    _query: &SyncQuery,
    filter: Option<&MatrixFilter>,
) -> Result<JoinedRoomResponse, Box<dyn std::error::Error + Send + Sync>> {
    // Get room information
    let room: Option<Room> = state.db.select(("room", room_id)).await?;
    let _room = room.ok_or("Room not found")?;

    // Extract room filter for this room
    let room_filter = filter.and_then(|f| f.room.as_ref());

    // Get timeline events with filter-aware database queries
    let timeline_events =
        if let Some(timeline_filter) = room_filter.and_then(|rf| rf.timeline.as_ref()) {
            get_filtered_timeline_events(state, room_id, timeline_filter).await?
        } else {
            get_default_timeline_events(state, room_id).await?
        };

    // Get state events
    let state_events = get_room_state_events(state, room_id).await?;

    // Get ephemeral events (read receipts, typing notifications)
    let ephemeral_events = get_room_ephemeral_events(state, room_id).await?;

    // Apply lazy loading filter if specified
    let filtered_timeline = if room_filter
        .and_then(|rf| rf.timeline.as_ref())
        .map(|tl| tl.lazy_load_members)
        .unwrap_or(false)
    {
        apply_lazy_loading_filter(timeline_events, room_id, user_id, state, false).await?
    } else {
        timeline_events
    };

    // Apply global event_fields filtering if specified
    let final_timeline = if let Some(event_fields) = filter.and_then(|f| f.event_fields.as_ref()) {
        apply_event_fields_filter(filtered_timeline, event_fields).await?
    } else {
        filtered_timeline
    };

    // Get room summary
    let heroes = get_room_heroes(state, room_id, user_id).await?;
    let joined_member_count = get_joined_member_count(state, room_id).await?;
    let invited_member_count = get_invited_member_count(state, room_id).await?;

    Ok(JoinedRoomResponse {
        summary: RoomSummary { heroes, joined_member_count, invited_member_count },
        state: StateResponse {
            events: convert_events_to_matrix_format(state_events),
        },
        timeline: TimelineResponse {
            events: convert_events_to_matrix_format(final_timeline),
            limited: false,
            prev_batch: None,
        },
        ephemeral: EphemeralResponse { events: ephemeral_events },
        account_data: AccountDataResponse {
            events: Vec::new(), // Room-specific account data would go here
        },
        unread_notifications: UnreadNotifications {
            highlight_count: 0, // TODO: Calculate actual counts
            notification_count: 0,
        },
    })
}

pub async fn build_invited_room_response(
    state: &AppState,
    room_id: &str,
) -> Result<InvitedRoomResponse, Box<dyn std::error::Error + Send + Sync>> {
    // Get invite state events
    let invite_state_events = get_room_state_events(state, room_id).await?;

    Ok(InvitedRoomResponse {
        invite_state: StateResponse {
            events: convert_events_to_matrix_format(invite_state_events),
        },
    })
}

pub async fn build_left_room_response(
    state: &AppState,
    room_id: &str,
    _user_id: &str,
) -> Result<LeftRoomResponse, Box<dyn std::error::Error + Send + Sync>> {
    // Get limited state for left rooms
    let state_events = get_room_state_events(state, room_id).await?;
    let timeline_events = get_default_timeline_events(state, room_id).await?;

    Ok(LeftRoomResponse {
        state: StateResponse {
            events: convert_events_to_matrix_format(state_events),
        },
        timeline: TimelineResponse {
            events: convert_events_to_matrix_format(timeline_events),
            limited: false,
            prev_batch: None,
        },
        account_data: AccountDataResponse {
            events: Vec::new(), // Room-specific account data for left rooms
        },
    })
}
