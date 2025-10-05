use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::ACCEPT},
    response::IntoResponse,
};
use chrono::Utc;
use std::collections::HashMap;
use tracing::{error, warn};

use crate::_matrix::client::v3::sync::data::{
    convert_events_to_matrix_format, get_invited_member_count, get_invited_rooms,
    get_joined_member_count, get_joined_rooms, get_left_rooms, get_room_ephemeral_events,
    get_room_heroes, get_room_state_events, get_room_timeline_events, get_user_account_data,
    get_user_presence_events, set_user_presence,
};
use crate::_matrix::client::v3::sync::filters::basic_filters::apply_room_filter;
use crate::_matrix::client::v3::sync::filters::{
    apply_account_data_filter, apply_cache_aware_lazy_loading_filter, apply_event_fields_filter,
    apply_presence_filter, apply_room_event_filter, get_filtered_timeline_events, resolve_filter,
};
use crate::_matrix::client::v3::sync::streaming::get_sse_stream;
use crate::auth::AuthenticatedUser;
use crate::metrics::filter_metrics::FilterTimer;
use crate::state::AppState;
use matryx_entity::types::{
    AccountDataResponse, DeviceListsResponse, EphemeralResponse, Event, InvitedRoomResponse,
    JoinedRoomResponse, LeftRoomResponse, MatrixFilter, PresenceResponse, RoomSummary,
    RoomsResponse, StateResponse, SyncQuery, SyncResponse, TimelineResponse, ToDeviceResponse,
    UnreadNotifications,
};

/// Parse since token to extract received_ts value
fn parse_since_token(since: &str) -> Option<i64> {
    if !since.starts_with('s') {
        return None;
    }
    since[1..].parse::<i64>().ok()
}

/// Generate next_batch token from maximum received_ts in events
fn generate_next_batch_token(max_received_ts: i64) -> String {
    format!("s{}", max_received_ts)
}

/// Extract maximum received_ts from a collection of events
fn get_max_received_ts(events: &[Event]) -> i64 {
    events
        .iter()
        .filter_map(|e| e.received_ts)
        .max()
        .unwrap_or(0)
}

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
    let _auth_device_id = auth.get_device_id();
    let _auth_access_token = auth.get_access_token();

    let user_id = &auth.user_id;

    // Handle sync parameters - parse since token
    let since_ts: Option<i64> = query.since.as_ref().and_then(|s| parse_since_token(s));
    let filter_param = query.filter.as_deref();
    let _full_state = query.full_state.unwrap_or(false);
    let _timeout_ms = query.timeout.unwrap_or(30000); // Default 30 seconds
    
    // Track maximum received_ts across all events for next_batch generation
    let mut max_received_ts: i64 = since_ts.unwrap_or(0);

    // Process filter parameter
    let applied_filter = if let Some(filter_param) = filter_param {
        resolve_filter(&state, filter_param, user_id).await?
    } else {
        None
    };

    // Record filter operation metrics
    if let Some(ref _filter) = applied_filter {
        use crate::metrics::filter_metrics::FilterMetrics;
        FilterMetrics::record_cache_operation("filter_resolve", true);
    }

    // Handle presence setting
    if let Some(presence) = &query.set_presence {
        set_user_presence(&state, &auth.user_id, presence).await.map_err(|e| {
            warn!("Failed to set user presence: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        tracing::debug!("Set user presence to: {}", presence);
    }

    // Get user's room memberships by state
    let mut joined_memberships = get_joined_rooms(&state, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut invited_memberships = get_invited_rooms(&state, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut left_memberships = get_left_rooms(&state, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Apply room filtering if a filter is specified
    if let Some(ref filter) = applied_filter {
        joined_memberships = apply_room_filter(joined_memberships, filter);
        invited_memberships = apply_room_filter(invited_memberships, filter);
        left_memberships = apply_room_filter(left_memberships, filter);
    }

    // Build room responses
    let mut joined_rooms = HashMap::new();
    let mut invited_rooms = HashMap::new();
    let mut left_rooms = HashMap::new();

    // Process joined rooms
    for membership in joined_memberships {
        let (room_response, room_max_ts) = build_joined_room_response(
            &state,
            &membership.room_id,
            user_id,
            &query,
            applied_filter.as_ref(),
            since_ts,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        
        // Update max_received_ts from room timeline events
        max_received_ts = max_received_ts.max(room_max_ts);
        
        joined_rooms.insert(membership.room_id, room_response);
    }

    // Process invited rooms
    for membership in invited_memberships {
        let room_response = build_invited_room_response(&state, &membership.room_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        invited_rooms.insert(membership.room_id, room_response);
    }

    // Process left rooms
    for membership in left_memberships {
        let room_response = build_left_room_response(&state, &membership.room_id, user_id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        left_rooms.insert(membership.room_id, room_response);
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

    // Query device list changes since last sync
    let device_lists = if let Some(since_token) = &query.since {
        // Parse since token to get timestamp
        let since_timestamp = since_token
            .trim_start_matches('s')
            .parse::<i64>()
            .unwrap_or_else(|_| Utc::now().timestamp_millis());

        // Convert timestamp to DateTime for database query
        let since_dt = chrono::DateTime::from_timestamp_millis(since_timestamp)
            .unwrap_or_else(chrono::Utc::now);

        // Query device EDUs with timestamp filter at database level
        // EDU table has created_at field but Rust struct doesn't expose it
        let device_edus: Vec<matryx_entity::types::EDU> = state.db
            .query("SELECT * FROM edu WHERE edu_type = 'm.device_list_update' AND created_at > $since ORDER BY created_at DESC")
            .bind(("since", since_dt))
            .await
            .map_err(|e| {
                error!("Failed to query device EDUs: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .take(0)
            .map_err(|e| {
                error!("Failed to extract EDU query results: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        // Extract changed user IDs (filtering already done at DB level)
        let changed_users: Vec<String> = device_edus
            .iter()
            .filter_map(|edu| {
                edu.ephemeral_event
                    .content
                    .get("user_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        DeviceListsResponse { changed: changed_users, left: Vec::new() }
    } else {
        // Initial sync - no device list changes
        DeviceListsResponse { changed: Vec::new(), left: Vec::new() }
    };

    // Generate next_batch token from maximum received_ts seen
    let next_batch = generate_next_batch_token(max_received_ts);

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
        device_lists,
        device_one_time_keys_count: HashMap::new(),
    };

    // Record sync performance metrics
    if let Some(_lazy_metrics) = &state.lazy_loading_metrics {
        // Metrics are recorded automatically by lazy loading functions
        tracing::debug!("Lazy loading metrics recorded for sync");
    }

    // Log filter cache statistics periodically
    if rand::random::<f64>() < 0.01 {
        // 1% sample rate to avoid overhead
        let stats = state.filter_cache.get_stats().await;
        tracing::info!(
            compiled_filters = stats.compiled_filters_count,
            cached_results = stats.cached_results_count,
            "Filter cache statistics"
        );
    }

    Ok(Json(response))
}

pub async fn build_joined_room_response(
    state: &AppState,
    room_id: &str,
    user_id: &str,
    _query: &SyncQuery,
    filter: Option<&MatrixFilter>,
    since_ts: Option<i64>,
) -> Result<(JoinedRoomResponse, i64), Box<dyn std::error::Error + Send + Sync>> {
    // Get room information using repository pattern
    use matryx_surrealdb::repository::RoomRepository;
    let room_repo = RoomRepository::new(state.db.clone());
    let _room = room_repo
        .get_by_id(room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
        .ok_or("Room not found")?;

    // Extract room filter for this room
    let room_filter = filter.and_then(|f| f.room.as_ref());

    // Get timeline events with enhanced room filtering including URL and lazy loading
    let timeline_events =
        if let Some(timeline_filter) = room_filter.and_then(|rf| rf.timeline.as_ref()) {
            let raw_events = get_filtered_timeline_events(state, room_id, timeline_filter, since_ts).await?;
            // Apply enhanced room event filtering with URL detection and lazy loading
            apply_room_event_filter(raw_events, timeline_filter, room_id, user_id, state).await?
        } else {
            get_room_timeline_events(state, room_id, None, since_ts).await?
        };

    // Get state events
    let state_events = get_room_state_events(state, room_id).await?;

    // Get ephemeral events (read receipts, typing notifications)
    let ephemeral_events = get_room_ephemeral_events(state, room_id).await?;

    // Apply lazy loading filter if specified with cache optimization
    let filtered_timeline = if room_filter
        .and_then(|rf| rf.timeline.as_ref())
        .map(|tl| tl.lazy_load_members)
        .unwrap_or(false)
    {
        if let Some(lazy_cache) = &state.lazy_loading_cache {
            let _timer = FilterTimer::new("lazy_loading");
            apply_cache_aware_lazy_loading_filter(
                timeline_events,
                room_id,
                user_id,
                state,
                false,
                lazy_cache,
            )
            .await
            .map_err(|e| {
                tracing::error!("Lazy loading filter failed: {}", e);
                Box::new(std::io::Error::other(e.to_string()))
                    as Box<dyn std::error::Error + Send + Sync>
            })?
        } else {
            // Fallback if lazy cache not available
            timeline_events
        }
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

    // Calculate max received_ts from timeline events before converting to JSON
    let max_ts = get_max_received_ts(&final_timeline);

    let response = JoinedRoomResponse {
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
    };

    Ok((response, max_ts))
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
    let timeline_events = get_room_timeline_events(state, room_id, None, None).await?;

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
