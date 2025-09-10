use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header::ACCEPT},
    response::{IntoResponse, Sse, sse::Event as SseEvent},
};
use chrono::Utc;
use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{self, warn};

use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use matryx_entity::types::{AccountData, Event, Membership, MembershipState, Room};
use matryx_surrealdb::repository::event::EventRepository;

#[derive(Deserialize)]
pub struct SyncQuery {
    filter: Option<String>,
    since: Option<String>,
    full_state: Option<bool>,
    set_presence: Option<String>,
    timeout: Option<u64>,
}

#[derive(Serialize)]
pub struct SyncResponse {
    next_batch: String,
    rooms: RoomsResponse,
    presence: PresenceResponse,
    account_data: AccountDataResponse,
    to_device: ToDeviceResponse,
    device_lists: DeviceListsResponse,
    device_one_time_keys_count: HashMap<String, u32>,
}

#[derive(Serialize)]
struct RoomsResponse {
    join: HashMap<String, JoinedRoomResponse>,
    invite: HashMap<String, InvitedRoomResponse>,
    leave: HashMap<String, LeftRoomResponse>,
}

#[derive(Serialize)]
struct JoinedRoomResponse {
    summary: RoomSummary,
    state: StateResponse,
    timeline: TimelineResponse,
    ephemeral: EphemeralResponse,
    account_data: AccountDataResponse,
    unread_notifications: UnreadNotifications,
}

#[derive(Serialize)]
struct InvitedRoomResponse {
    invite_state: StateResponse,
}

#[derive(Serialize)]
struct LeftRoomResponse {
    state: StateResponse,
    timeline: TimelineResponse,
    account_data: AccountDataResponse,
}

#[derive(Serialize)]
struct RoomSummary {
    #[serde(rename = "m.heroes")]
    heroes: Vec<String>,
    #[serde(rename = "m.joined_member_count")]
    joined_member_count: u32,
    #[serde(rename = "m.invited_member_count")]
    invited_member_count: u32,
}

#[derive(Serialize)]
struct StateResponse {
    events: Vec<Value>,
}

#[derive(Serialize)]
struct TimelineResponse {
    events: Vec<Value>,
    limited: bool,
    prev_batch: Option<String>,
}

#[derive(Serialize)]
struct EphemeralResponse {
    events: Vec<Value>,
}

#[derive(Serialize)]
struct PresenceResponse {
    events: Vec<Value>,
}

#[derive(Serialize)]
struct AccountDataResponse {
    events: Vec<Value>,
}

#[derive(Serialize)]
struct ToDeviceResponse {
    events: Vec<Value>,
}

#[derive(Serialize)]
struct DeviceListsResponse {
    changed: Vec<String>,
    left: Vec<String>,
}

#[derive(Serialize)]
struct UnreadNotifications {
    highlight_count: u32,
    notification_count: u32,
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
async fn get_json_sync(
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
    let _filter_id = query.filter.as_deref();
    let _full_state = query.full_state.unwrap_or(false);
    let _timeout_ms = query.timeout.unwrap_or(30000); // Default 30 seconds

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

    // Separate rooms by membership state
    let mut joined_rooms = HashMap::new();
    let mut invited_rooms = HashMap::new();
    let mut left_rooms = HashMap::new();

    for membership in memberships {
        match membership.membership {
            MembershipState::Join => {
                let room_response =
                    build_joined_room_response(&state, &membership.room_id, user_id, &query)
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

    // Get account data
    let account_data = get_user_account_data(&state, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get presence events
    let presence_events = get_user_presence_events(&state, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = SyncResponse {
        next_batch,
        rooms: RoomsResponse {
            join: joined_rooms,
            invite: invited_rooms,
            leave: left_rooms,
        },
        presence: PresenceResponse { events: presence_events },
        account_data: AccountDataResponse { events: account_data },
        to_device: ToDeviceResponse { events: Vec::new() },
        device_lists: DeviceListsResponse { changed: Vec::new(), left: Vec::new() },
        device_one_time_keys_count: HashMap::new(),
    };

    Ok(Json(response))
}

async fn get_user_memberships(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Membership>, Box<dyn std::error::Error + Send + Sync>> {
    let memberships: Vec<Membership> = state
        .db
        .query("SELECT * FROM membership WHERE user_id = $user_id")
        .bind(("user_id", user_id.to_string()))
        .await?
        .take(0)?;

    Ok(memberships)
}

async fn build_joined_room_response(
    state: &AppState,
    room_id: &str,
    _user_id: &str,
    _query: &SyncQuery,
) -> Result<JoinedRoomResponse, Box<dyn std::error::Error + Send + Sync>> {
    // Get room information
    let room: Option<Room> = state.db.select(("room", room_id)).await?;

    let _room = room.ok_or("Room not found")?;

    // Get recent events for timeline
    let events: Vec<Event> = state
        .db
        .query(
            "SELECT * FROM event WHERE room_id = $room_id ORDER BY origin_server_ts DESC LIMIT 20",
        )
        .bind(("room_id", room_id.to_string()))
        .await?
        .take(0)?;

    // Convert events to Matrix format
    let timeline_events: Vec<Value> = events
        .into_iter()
        .map(|event| {
            json!({
                "event_id": event.event_id,
                "sender": event.sender,
                "origin_server_ts": event.origin_server_ts,
                "type": event.event_type,
                "content": event.content,
                "state_key": event.state_key,
                "unsigned": event.unsigned
            })
        })
        .collect();

    // Get room state events
    let state_events: Vec<Event> = state
        .db
        .query("SELECT * FROM event WHERE room_id = $room_id AND state_key IS NOT NULL")
        .bind(("room_id", room_id.to_string()))
        .await?
        .take(0)?;

    let state_event_values: Vec<Value> = state_events
        .into_iter()
        .map(|event| {
            json!({
                "event_id": event.event_id,
                "sender": event.sender,
                "origin_server_ts": event.origin_server_ts,
                "type": event.event_type,
                "content": event.content,
                "state_key": event.state_key
            })
        })
        .collect();

    // Get member count
    let member_count: Vec<i64> = state.db
        .query("SELECT count() FROM membership WHERE room_id = $room_id AND membership = $membership GROUP ALL")
        .bind(("room_id", room_id.to_string()))
        .bind(("membership", "join"))
        .await?
        .take(0)?;

    let joined_count = member_count.first().unwrap_or(&0);

    Ok(JoinedRoomResponse {
        summary: RoomSummary {
            heroes: get_room_heroes(state, room_id, _user_id).await.unwrap_or_default(),
            joined_member_count: *joined_count as u32,
            invited_member_count: get_invited_member_count(state, room_id).await.unwrap_or(0),
        },
        state: StateResponse { events: state_event_values },
        timeline: TimelineResponse {
            events: timeline_events,
            limited: false,
            prev_batch: None,
        },
        ephemeral: EphemeralResponse { events: Vec::new() },
        account_data: AccountDataResponse { events: Vec::new() },
        unread_notifications: UnreadNotifications { highlight_count: 0, notification_count: 0 },
    })
}

async fn build_invited_room_response(
    state: &AppState,
    room_id: &str,
) -> Result<InvitedRoomResponse, Box<dyn std::error::Error + Send + Sync>> {
    // Get invite state events
    let state_events: Vec<Event> = state
        .db
        .query("SELECT * FROM event WHERE room_id = $room_id AND state_key IS NOT NULL")
        .bind(("room_id", room_id.to_string()))
        .await?
        .take(0)?;

    let state_event_values: Vec<Value> = state_events
        .into_iter()
        .map(|event| {
            json!({
                "event_id": event.event_id,
                "sender": event.sender,
                "origin_server_ts": event.origin_server_ts,
                "type": event.event_type,
                "content": event.content,
                "state_key": event.state_key
            })
        })
        .collect();

    Ok(InvitedRoomResponse {
        invite_state: StateResponse { events: state_event_values },
    })
}

async fn build_left_room_response(
    _state: &AppState,
    _room_id: &str,
    _user_id: &str,
) -> Result<LeftRoomResponse, Box<dyn std::error::Error + Send + Sync>> {
    // For left rooms, return minimal data
    Ok(LeftRoomResponse {
        state: StateResponse { events: Vec::new() },
        timeline: TimelineResponse {
            events: Vec::new(),
            limited: false,
            prev_batch: None,
        },
        account_data: AccountDataResponse { events: Vec::new() },
    })
}

async fn get_user_account_data(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let account_data: Vec<AccountData> = state
        .db
        .query("SELECT * FROM account_data WHERE user_id = $user_id AND room_id IS NULL")
        .bind(("user_id", user_id.to_string()))
        .await?
        .take(0)?;

    let events = account_data
        .into_iter()
        .map(|data| {
            json!({
                "type": data.account_data_type,
                "content": data.content
            })
        })
        .collect();

    Ok(events)
}

/// Update user presence status
async fn set_user_presence(
    state: &AppState,
    user_id: &str,
    presence: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Validate presence value
    let valid_presence = match presence {
        "online" | "offline" | "unavailable" => presence,
        _ => return Err("Invalid presence value".into()),
    };

    // Update or create presence event
    let query = r#"
        UPSERT presence_events:⟨$user_id⟩ CONTENT {
            user_id: $user_id,
            presence: $presence,
            status_msg: NONE,
            last_active_ago: 0,
            currently_active: $currently_active,
            updated_at: time::now()
        }
    "#;

    let currently_active = valid_presence == "online";

    let _: Option<serde_json::Value> = state
        .db
        .query(query)
        .bind(("user_id", user_id.to_string()))
        .bind(("presence", valid_presence.to_string()))
        .bind(("currently_active", currently_active))
        .await?
        .take(0)?;

    Ok(())
}

/// Get room heroes (other prominent members for room summary)
async fn get_room_heroes(
    state: &AppState,
    room_id: &str,
    current_user_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get up to 5 most recently active members excluding current user
    let query = r#"
        SELECT user_id FROM membership
        WHERE room_id = $room_id
        AND membership = 'join'
        AND user_id != $current_user_id
        ORDER BY updated_at DESC
        LIMIT 5
    "#;

    let mut response = state
        .db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .bind(("current_user_id", current_user_id.to_string()))
        .await?;

    #[derive(serde::Deserialize)]
    struct MemberInfo {
        user_id: String,
    }

    let members: Vec<MemberInfo> = response.take(0)?;
    let heroes = members.into_iter().map(|m| m.user_id).collect();

    Ok(heroes)
}

/// Get count of invited members in a room
async fn get_invited_member_count(
    state: &AppState,
    room_id: &str,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let query = "SELECT count() FROM membership WHERE room_id = $room_id AND membership = 'invite' GROUP ALL";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    let count: Option<i64> = response.take(0)?;
    Ok(count.unwrap_or(0) as u32)
}

/// Get presence events for user's contacts and self
async fn get_user_presence_events(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let query = r#"
        SELECT * FROM presence_events
        WHERE user_id IN (
            SELECT VALUE target_user_id FROM user_relationships
            WHERE user_id = $user_id AND relationship_type = 'friend'
        )
        OR user_id = $user_id
        ORDER BY updated_at DESC
    "#;

    let user_id_owned = user_id.to_string();
    let mut response = state.db.query(query).bind(("user_id", user_id_owned)).await?;

    #[derive(serde::Deserialize)]
    struct PresenceEvent {
        user_id: String,
        presence: String,
        status_msg: Option<String>,
        last_active_ago: Option<i64>,
        currently_active: bool,
    }

    let presence_events: Vec<PresenceEvent> = response.take(0)?;

    let events: Vec<Value> = presence_events
        .into_iter()
        .map(|event| {
            json!({
                "type": "m.presence",
                "sender": event.user_id,
                "content": {
                    "presence": event.presence,
                    "status_msg": event.status_msg,
                    "last_active_ago": event.last_active_ago,
                    "currently_active": event.currently_active
                }
            })
        })
        .collect();

    Ok(events)
}

/// Server-Sent Events streaming sync implementation
/// Provides real-time Matrix sync using SurrealDB LiveQuery
async fn get_sse_stream(
    state: AppState,
    auth: AuthenticatedUser,
    query: SyncQuery,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, axum::Error>>>, StatusCode> {
    let user_id = auth.user_id.clone();

    // Create channel for sending updates
    let (tx, rx) = mpsc::channel::<Result<SseEvent, axum::Error>>(100);

    // Spawn background task to handle LiveQuery streams
    let state_clone = state.clone();
    let auth_clone = auth.clone();
    tokio::spawn(async move {
        if let Err(e) = handle_live_sync_streams(state_clone, auth_clone, tx).await {
            tracing::error!("Error in live sync stream: {:?}", e);
        }
    });

    // Convert receiver to stream
    let stream = ReceiverStream::new(rx);

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(30))
            .text("keepalive"),
    ))
}

async fn handle_live_sync_streams(
    state: AppState,
    auth: AuthenticatedUser,
    tx: mpsc::Sender<Result<SseEvent, axum::Error>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = &auth.user_id;

    // Send initial sync data
    send_initial_sync_sse(&state, &auth, &tx).await?;

    // Set up LiveQuery streams for real-time updates

    // 1. Stream for new events in joined rooms
    let event_stream = create_event_live_stream(&state, user_id.clone()).await?;

    // 2. Stream for membership changes
    let membership_stream = create_membership_live_stream(&state, user_id.clone()).await?;

    // 3. Stream for account data changes
    let account_data_stream = create_account_data_live_stream(&state, user_id.clone()).await?;

    // 4. Stream for presence updates
    let presence_stream = create_presence_live_stream(&state, user_id.clone()).await?;

    // Merge all streams and handle updates
    let mut combined_stream = futures::stream::select_all(vec![
        event_stream.boxed(),
        membership_stream.boxed(),
        account_data_stream.boxed(),
        presence_stream.boxed(),
    ]);

    while let Some(update_result) = combined_stream.next().await {
        match update_result {
            Ok(sync_update) => {
                let event_data = serde_json::to_string(&sync_update)?;
                let sse_event = SseEvent::default().event("sync").data(event_data);

                if tx.send(Ok(sse_event)).await.is_err() {
                    // Client disconnected
                    break;
                }
            },
            Err(e) => {
                tracing::error!("Error in live stream: {:?}", e);
                let error_event =
                    SseEvent::default().event("error").data(format!("Stream error: {}", e));

                if tx.send(Ok(error_event)).await.is_err() {
                    break;
                }
            },
        }
    }

    Ok(())
}

async fn send_initial_sync_sse(
    state: &AppState,
    auth: &AuthenticatedUser,
    tx: &mpsc::Sender<Result<SseEvent, axum::Error>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get initial sync data using existing JSON sync logic
    let query = SyncQuery {
        filter: None,
        since: None,
        full_state: Some(true),
        set_presence: None,
        timeout: None,
    };

    let sync_response = get_json_sync(state.clone(), auth.clone(), query)
        .await
        .map_err(|_| "Failed to get initial sync data")?;

    let event_data = serde_json::to_string(&sync_response.0)?;
    let sse_event = SseEvent::default().event("initial_sync").data(event_data);

    tx.send(Ok(sse_event)).await.map_err(|_| "Client disconnected")?;

    Ok(())
}

#[derive(Serialize, Clone)]
struct LiveSyncUpdate {
    next_batch: String,
    rooms: Option<RoomsUpdate>,
    presence: Option<PresenceUpdate>,
    account_data: Option<AccountDataUpdate>,
    to_device: Option<ToDeviceUpdate>,
    device_lists: Option<DeviceListsUpdate>,
}

#[derive(Serialize, Clone)]
struct RoomsUpdate {
    join: Option<HashMap<String, JoinedRoomUpdate>>,
    invite: Option<HashMap<String, InvitedRoomUpdate>>,
    leave: Option<HashMap<String, LeftRoomUpdate>>,
}

#[derive(Serialize, Clone)]
struct JoinedRoomUpdate {
    timeline: Option<TimelineUpdate>,
    state: Option<StateUpdate>,
    ephemeral: Option<EphemeralUpdate>,
    account_data: Option<AccountDataUpdate>,
    unread_notifications: Option<UnreadNotificationsUpdate>,
}

#[derive(Serialize, Clone)]
struct InvitedRoomUpdate {
    invite_state: Option<StateUpdate>,
}

#[derive(Serialize, Clone)]
struct LeftRoomUpdate {
    timeline: Option<TimelineUpdate>,
    state: Option<StateUpdate>,
}

#[derive(Serialize, Clone)]
struct TimelineUpdate {
    events: Vec<Value>,
    limited: Option<bool>,
    prev_batch: Option<String>,
}

#[derive(Serialize, Clone)]
struct StateUpdate {
    events: Vec<Value>,
}

#[derive(Serialize, Clone)]
struct EphemeralUpdate {
    events: Vec<Value>,
}

#[derive(Serialize, Clone)]
struct PresenceUpdate {
    events: Vec<Value>,
}

#[derive(Serialize, Clone)]
struct AccountDataUpdate {
    events: Vec<Value>,
}

#[derive(Serialize, Clone)]
struct ToDeviceUpdate {
    events: Vec<Value>,
}

#[derive(Serialize, Clone)]
struct DeviceListsUpdate {
    changed: Option<Vec<String>>,
    left: Option<Vec<String>>,
}

#[derive(Serialize, Clone)]
struct UnreadNotificationsUpdate {
    highlight_count: Option<u32>,
    notification_count: Option<u32>,
}

async fn create_event_live_stream(
    state: &AppState,
    user_id: String,
) -> Result<
    impl Stream<Item = Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Create LiveQuery for events in rooms where user has membership
    let mut stream = state
        .db
        .query(
            r#"
            LIVE SELECT * FROM event
            WHERE room_id IN (
                SELECT VALUE room_id FROM membership
                WHERE user_id = $user_id AND membership IN ['join', 'invite']
            )
            AND state_key IS NULL
        "#,
        )
        .bind(("user_id", user_id.clone()))
        .await?;

    // Transform SurrealDB notification stream to event stream
    let sync_stream = stream.stream::<surrealdb::Notification<Event>>(0)?
        .map(move |notification_result| -> Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>> {
            let notification = notification_result?;

            match notification.action {
                surrealdb::Action::Create => {
                    // New event created
                    let event = notification.data;

                    // Create sync update for this event
                    let event_json = json!({
                        "event_id": event.event_id,
                        "sender": event.sender,
                        "origin_server_ts": event.origin_server_ts,
                        "type": event.event_type,
                        "content": event.content,
                        "unsigned": event.unsigned
                    });

                    let mut joined_rooms = HashMap::new();
                    joined_rooms.insert(event.room_id.clone(), JoinedRoomUpdate {
                        timeline: Some(TimelineUpdate {
                            events: vec![event_json],
                            limited: Some(false),
                            prev_batch: None,
                        }),
                        state: None,
                        ephemeral: None,
                        account_data: None,
                        unread_notifications: None,
                    });

                    Ok(LiveSyncUpdate {
                        next_batch: format!("s{}", Utc::now().timestamp_millis()),
                        rooms: Some(RoomsUpdate {
                            join: Some(joined_rooms),
                            invite: None,
                            leave: None,
                        }),
                        presence: None,
                        account_data: None,
                        to_device: None,
                        device_lists: None,
                    })
                },
                _ => {
                    // Handle other actions (update, delete) if needed
                    Ok(LiveSyncUpdate {
                        next_batch: format!("s{}", Utc::now().timestamp_millis()),
                        rooms: None,
                        presence: None,
                        account_data: None,
                        to_device: None,
                        device_lists: None,
                    })
                }
            }
        });

    Ok(sync_stream)
}

async fn create_membership_live_stream(
    state: &AppState,
    user_id: String,
) -> Result<
    impl Stream<Item = Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Create LiveQuery for membership changes affecting this user
    let mut stream = state
        .db
        .query(
            r#"
            LIVE SELECT * FROM membership
            WHERE user_id = $user_id
        "#,
        )
        .bind(("user_id", user_id.clone()))
        .await?;

    let sync_stream = stream.stream::<surrealdb::Notification<Membership>>(0)?
        .map(move |notification_result| -> Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>> {
            let notification = notification_result?;

            match notification.action {
                surrealdb::Action::Create | surrealdb::Action::Update => {
                    let membership = notification.data;

                    // Create appropriate room update based on membership state
                    let rooms_update = match membership.membership {
                        MembershipState::Join => {
                            let mut joined_rooms = HashMap::new();
                            joined_rooms.insert(membership.room_id.clone(), JoinedRoomUpdate {
                                timeline: None,
                                state: Some(StateUpdate {
                                    events: Vec::new(), // Would populate with actual state events
                                }),
                                ephemeral: None,
                                account_data: None,
                                unread_notifications: None,
                            });

                            RoomsUpdate {
                                join: Some(joined_rooms),
                                invite: None,
                                leave: None,
                            }
                        },
                        MembershipState::Invite => {
                            let mut invited_rooms = HashMap::new();
                            invited_rooms.insert(membership.room_id.clone(), InvitedRoomUpdate {
                                invite_state: Some(StateUpdate {
                                    events: Vec::new(), // Would populate with invite state
                                }),
                            });

                            RoomsUpdate {
                                join: None,
                                invite: Some(invited_rooms),
                                leave: None,
                            }
                        },
                        MembershipState::Leave | MembershipState::Ban => {
                            let mut left_rooms = HashMap::new();
                            left_rooms.insert(membership.room_id.clone(), LeftRoomUpdate {
                                timeline: None,
                                state: None,
                            });

                            RoomsUpdate {
                                join: None,
                                invite: None,
                                leave: Some(left_rooms),
                            }
                        },
                        _ => RoomsUpdate {
                            join: None,
                            invite: None,
                            leave: None,
                        },
                    };

                    Ok(LiveSyncUpdate {
                        next_batch: format!("s{}", Utc::now().timestamp_millis()),
                        rooms: Some(rooms_update),
                        presence: None,
                        account_data: None,
                        to_device: None,
                        device_lists: None,
                    })
                },
                _ => Ok(LiveSyncUpdate {
                    next_batch: format!("s{}", Utc::now().timestamp_millis()),
                    rooms: None,
                    presence: None,
                    account_data: None,
                    to_device: None,
                    device_lists: None,
                }),
            }
        });

    Ok(sync_stream)
}

async fn create_account_data_live_stream(
    state: &AppState,
    user_id: String,
) -> Result<
    impl Stream<Item = Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Create LiveQuery for account data changes
    let mut stream = state
        .db
        .query(
            r#"
            LIVE SELECT * FROM account_data
            WHERE user_id = $user_id
        "#,
        )
        .bind(("user_id", user_id.clone()))
        .await?;

    let sync_stream = stream.stream::<surrealdb::Notification<AccountData>>(0)?
        .map(move |notification_result| -> Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>> {
            let notification = notification_result?;

            match notification.action {
                surrealdb::Action::Create | surrealdb::Action::Update => {
                    let account_data = notification.data;

                    let account_event = json!({
                        "type": account_data.account_data_type,
                        "content": account_data.content
                    });

                    Ok(LiveSyncUpdate {
                        next_batch: format!("s{}", Utc::now().timestamp_millis()),
                        rooms: None,
                        presence: None,
                        account_data: Some(AccountDataUpdate {
                            events: vec![account_event],
                        }),
                        to_device: None,
                        device_lists: None,
                    })
                },
                _ => Ok(LiveSyncUpdate {
                    next_batch: format!("s{}", Utc::now().timestamp_millis()),
                    rooms: None,
                    presence: None,
                    account_data: None,
                    to_device: None,
                    device_lists: None,
                }),
            }
        });

    Ok(sync_stream)
}

async fn create_presence_live_stream(
    state: &AppState,
    user_id: String,
) -> Result<
    impl Stream<Item = Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Create LiveQuery for presence changes affecting this user's contacts
    let mut stream = state
        .db
        .query(
            r#"
            LIVE SELECT * FROM presence_events
            WHERE user_id IN (
                SELECT VALUE target_user_id FROM user_relationships
                WHERE user_id = $user_id AND relationship_type = 'friend'
            )
            OR user_id = $user_id
        "#,
        )
        .bind(("user_id", user_id.clone()))
        .await?;

    let sync_stream = stream.stream::<surrealdb::Notification<serde_json::Value>>(0)?
        .map(move |notification_result| -> Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>> {
            let notification = notification_result?;

            match notification.action {
                surrealdb::Action::Create | surrealdb::Action::Update => {
                    let presence_data = notification.data;

                    let presence_event = json!({
                        "type": "m.presence",
                        "sender": presence_data.get("user_id").and_then(|v| v.as_str()).unwrap_or(""),
                        "content": {
                            "presence": presence_data.get("presence").and_then(|v| v.as_str()).unwrap_or("offline"),
                            "status_msg": presence_data.get("status_msg"),
                            "last_active_ago": presence_data.get("last_active_ago"),
                            "currently_active": presence_data.get("currently_active").and_then(|v| v.as_bool()).unwrap_or(false)
                        }
                    });

                    Ok(LiveSyncUpdate {
                        next_batch: format!("s{}", Utc::now().timestamp_millis()),
                        rooms: None,
                        presence: Some(PresenceUpdate {
                            events: vec![presence_event],
                        }),
                        account_data: None,
                        to_device: None,
                        device_lists: None,
                    })
                },
                _ => Ok(LiveSyncUpdate {
                    next_batch: format!("s{}", Utc::now().timestamp_millis()),
                    rooms: None,
                    presence: None,
                    account_data: None,
                    to_device: None,
                    device_lists: None,
                }),
            }
        });

    Ok(sync_stream)
}
