use axum::{
    extract::{
        Query,
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::Response,
};
use chrono::{DateTime, Utc};
use futures::stream::{SplitSink, SplitStream, Stream, StreamExt};
use futures::{SinkExt, select};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use matryx_entity::types::{AccountData, Event, Membership, MembershipState, Room};

#[derive(Deserialize)]
pub struct SyncWebSocketQuery {
    filter: Option<String>,
    timeout: Option<u64>,
    set_presence: Option<String>,
}

#[derive(Serialize, Clone)]
struct WebSocketSyncUpdate {
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
struct AccountDataUpdate {
    events: Vec<Value>,
}

#[derive(Serialize, Clone)]
struct PresenceUpdate {
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

#[derive(Serialize)]
struct WebSocketMessage {
    #[serde(rename = "type")]
    message_type: String,
    data: Value,
}

#[derive(Serialize)]
struct PingMessage {
    #[serde(rename = "type")]
    message_type: String,
    timestamp: i64,
}

#[derive(Deserialize)]
struct PongMessage {
    #[serde(rename = "type")]
    message_type: String,
    timestamp: i64,
}

/// WebSocket endpoint for Matrix sync with LiveQuery support
/// Provides real-time Matrix sync over WebSocket transport
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    auth: AuthenticatedUser,
    Query(query): Query<SyncWebSocketQuery>,
) -> Result<Response, StatusCode> {
    info!("WebSocket connection request from user: {}", auth.user_id);

    Ok(ws.on_upgrade(move |socket| handle_websocket_connection(socket, state, auth, query)))
}

async fn handle_websocket_connection(
    socket: WebSocket,
    state: AppState,
    auth: AuthenticatedUser,
    query: SyncWebSocketQuery,
) {
    let user_id = auth.user_id.clone();
    info!("WebSocket connection established for user: {}", user_id);

    let (sender, receiver) = socket.split();
    let (tx, rx) = mpsc::channel::<Message>(100);

    // Spawn task to handle outgoing messages
    let send_task = tokio::spawn(handle_outgoing_messages(sender, rx));

    // Spawn task to handle incoming messages (ping/pong, control messages)
    let recv_task = tokio::spawn(handle_incoming_messages(receiver, tx.clone()));

    // Spawn task to handle sync streams
    let sync_task =
        tokio::spawn(handle_sync_streams(state.clone(), auth.clone(), tx.clone(), query));

    // Spawn task to handle periodic ping
    let ping_task = tokio::spawn(handle_ping(tx.clone()));

    // Wait for any task to complete (indicating connection should close)
    tokio::select! {
        result = send_task => {
            if let Err(e) = result {
                error!("Send task failed: {:?}", e);
            }
        }
        result = recv_task => {
            if let Err(e) = result {
                error!("Receive task failed: {:?}", e);
            }
        }
        result = sync_task => {
            if let Err(e) = result {
                error!("Sync task failed: {:?}", e);
            }
        }
        result = ping_task => {
            if let Err(e) = result {
                error!("Ping task failed: {:?}", e);
            }
        }
    }

    info!("WebSocket connection closed for user: {}", user_id);
}

async fn handle_outgoing_messages(
    mut sender: SplitSink<WebSocket, Message>,
    mut rx: mpsc::Receiver<Message>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    while let Some(message) = rx.recv().await {
        if sender.send(message).await.is_err() {
            break;
        }
    }
    Ok(())
}

async fn handle_incoming_messages(
    mut receiver: SplitStream<WebSocket>,
    tx: mpsc::Sender<Message>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Handle JSON control messages
                if let Ok(pong) = serde_json::from_str::<PongMessage>(&text) {
                    if pong.message_type == "pong" {
                        debug!("Received pong from client at {}", pong.timestamp);
                    }
                }
            },
            Ok(Message::Pong(_)) => {
                debug!("Received WebSocket pong");
            },
            Ok(Message::Close(_)) => {
                info!("WebSocket close message received");
                break;
            },
            Err(e) => {
                warn!("WebSocket message error: {:?}", e);
                break;
            },
            _ => {},
        }
    }
    Ok(())
}

async fn handle_ping(
    tx: mpsc::Sender<Message>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut ping_interval = interval(Duration::from_secs(30));

    loop {
        ping_interval.tick().await;

        let ping_msg = PingMessage {
            message_type: "ping".to_string(),
            timestamp: Utc::now().timestamp_millis(),
        };

        let message_json = serde_json::to_string(&ping_msg)?;
        let message = Message::Text(message_json.into());

        if tx.send(message).await.is_err() {
            break;
        }
    }

    Ok(())
}

async fn handle_sync_streams(
    state: AppState,
    auth: AuthenticatedUser,
    tx: mpsc::Sender<Message>,
    query: SyncWebSocketQuery,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = &auth.user_id;

    // Send initial sync data
    send_initial_sync_ws(&state, &auth, &tx).await?;

    // Set up LiveQuery streams for real-time updates (reuse from sync_live.rs)
    let event_stream = create_event_live_stream_ws(&state, user_id).await?;
    let membership_stream = create_membership_live_stream_ws(&state, user_id).await?;
    let account_data_stream = create_account_data_live_stream_ws(&state, user_id).await?;

    // Merge all streams and handle updates
    let mut combined_stream = futures::stream::select_all(vec![
        event_stream.boxed(),
        membership_stream.boxed(),
        account_data_stream.boxed(),
    ]);

    while let Some(update_result) = combined_stream.next().await {
        match update_result {
            Ok(sync_update) => {
                let ws_message = WebSocketMessage {
                    message_type: "sync".to_string(),
                    data: serde_json::to_value(&sync_update)?,
                };

                let message_json = serde_json::to_string(&ws_message)?;
                let message = Message::Text(message_json.into());

                if tx.send(message).await.is_err() {
                    // Client disconnected
                    break;
                }
            },
            Err(e) => {
                error!("Error in live stream: {:?}", e);
                let error_message = WebSocketMessage {
                    message_type: "error".to_string(),
                    data: json!({ "error": format!("Stream error: {}", e) }),
                };

                let message_json = serde_json::to_string(&error_message)?;
                let message = Message::Text(message_json.into());

                if tx.send(message).await.is_err() {
                    break;
                }
            },
        }
    }

    Ok(())
}

async fn send_initial_sync_ws(
    state: &AppState,
    auth: &AuthenticatedUser,
    tx: &mpsc::Sender<Message>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get initial sync data (reuse logic from sync_live.rs)
    let memberships = get_user_memberships_ws(state, &auth.user_id).await?;
    let mut joined_rooms = HashMap::new();
    let mut invited_rooms = HashMap::new();
    let mut left_rooms = HashMap::new();

    for membership in memberships {
        match membership.membership {
            MembershipState::Join => {
                let room_update =
                    build_initial_joined_room_update_ws(state, &membership.room_id, &auth.user_id)
                        .await?;
                joined_rooms.insert(membership.room_id, room_update);
            },
            MembershipState::Invite => {
                let room_update =
                    build_initial_invited_room_update_ws(state, &membership.room_id).await?;
                invited_rooms.insert(membership.room_id, room_update);
            },
            MembershipState::Leave => {
                let room_update =
                    build_initial_left_room_update_ws(state, &membership.room_id).await?;
                left_rooms.insert(membership.room_id, room_update);
            },
            _ => {},
        }
    }

    let initial_sync = WebSocketSyncUpdate {
        next_batch: format!("s{}", Utc::now().timestamp_millis()),
        rooms: Some(RoomsUpdate {
            join: if joined_rooms.is_empty() {
                None
            } else {
                Some(joined_rooms)
            },
            invite: if invited_rooms.is_empty() {
                None
            } else {
                Some(invited_rooms)
            },
            leave: if left_rooms.is_empty() {
                None
            } else {
                Some(left_rooms)
            },
        }),
        presence: None,
        account_data: None,
        to_device: None,
        device_lists: None,
    };

    let ws_message = WebSocketMessage {
        message_type: "initial_sync".to_string(),
        data: serde_json::to_value(&initial_sync)?,
    };

    let message_json = serde_json::to_string(&ws_message)?;
    let message = Message::Text(message_json.into());

    tx.send(message).await.map_err(|_| "Client disconnected")?;

    Ok(())
}

// LiveQuery stream creation functions (adapted from sync_live.rs)
async fn create_event_live_stream_ws(
    state: &AppState,
    user_id: &str,
) -> Result<
    impl Stream<Item = Result<WebSocketSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let mut stream = state
        .db
        .query(
            r#"
            LIVE SELECT * FROM event
            WHERE room_id IN (
                SELECT VALUE room_id FROM membership
                WHERE user_id = $user_id AND membership = 'join'
            )
            AND state_key IS NULL
        "#,
        )
        .bind(("user_id", user_id.to_string()))
        .await?;

    let sync_stream = stream.stream::<surrealdb::Notification<Event>>(0)?
        .map(move |notification_result| -> Result<WebSocketSyncUpdate, Box<dyn std::error::Error + Send + Sync>> {
            let notification = notification_result?;

            match notification.action {
                surrealdb::Action::Create => {
                    let event = notification.data;
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

                    Ok(WebSocketSyncUpdate {
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
                    Ok(WebSocketSyncUpdate {
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

async fn create_membership_live_stream_ws(
    state: &AppState,
    user_id: &str,
) -> Result<
    impl Stream<Item = Result<WebSocketSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let mut stream = state
        .db
        .query(
            r#"
            LIVE SELECT * FROM membership
            WHERE user_id = $user_id
        "#,
        )
        .bind(("user_id", user_id.to_string()))
        .await?;

    let sync_stream = stream.stream::<surrealdb::Notification<Membership>>(0)?
        .map(move |notification_result| -> Result<WebSocketSyncUpdate, Box<dyn std::error::Error + Send + Sync>> {
            let notification = notification_result?;

            match notification.action {
                surrealdb::Action::Create | surrealdb::Action::Update => {
                    let membership = notification.data;

                    let rooms_update = match membership.membership {
                        MembershipState::Join => {
                            let mut joined_rooms = HashMap::new();
                            joined_rooms.insert(membership.room_id.clone(), JoinedRoomUpdate {
                                timeline: None,
                                state: None,
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
                                invite_state: None,
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

                    Ok(WebSocketSyncUpdate {
                        next_batch: format!("s{}", Utc::now().timestamp_millis()),
                        rooms: Some(rooms_update),
                        presence: None,
                        account_data: None,
                        to_device: None,
                        device_lists: None,
                    })
                },
                _ => Ok(WebSocketSyncUpdate {
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

async fn create_account_data_live_stream_ws(
    state: &AppState,
    user_id: &str,
) -> Result<
    impl Stream<Item = Result<WebSocketSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let mut stream = state
        .db
        .query(
            r#"
            LIVE SELECT * FROM account_data
            WHERE user_id = $user_id
        "#,
        )
        .bind(("user_id", user_id.to_string()))
        .await?;

    let sync_stream = stream.stream::<surrealdb::Notification<AccountData>>(0)?
        .map(move |notification_result| -> Result<WebSocketSyncUpdate, Box<dyn std::error::Error + Send + Sync>> {
            let notification = notification_result?;

            match notification.action {
                surrealdb::Action::Create | surrealdb::Action::Update => {
                    let account_data = notification.data;

                    let account_event = json!({
                        "type": account_data.account_data_type,
                        "content": account_data.content
                    });

                    Ok(WebSocketSyncUpdate {
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
                _ => Ok(WebSocketSyncUpdate {
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

// Helper functions (adapted from sync_live.rs)
async fn get_user_memberships_ws(
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

async fn build_initial_joined_room_update_ws(
    state: &AppState,
    room_id: &str,
    user_id: &str,
) -> Result<JoinedRoomUpdate, Box<dyn std::error::Error + Send + Sync>> {
    let events: Vec<Event> = state
        .db
        .query(
            "SELECT * FROM event WHERE room_id = $room_id ORDER BY origin_server_ts DESC LIMIT 20",
        )
        .bind(("room_id", room_id.to_string()))
        .await?
        .take(0)?;

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

    Ok(JoinedRoomUpdate {
        timeline: Some(TimelineUpdate {
            events: timeline_events,
            limited: Some(false),
            prev_batch: None,
        }),
        state: None,
        ephemeral: None,
        account_data: None,
        unread_notifications: None,
    })
}

async fn build_initial_invited_room_update_ws(
    state: &AppState,
    room_id: &str,
) -> Result<InvitedRoomUpdate, Box<dyn std::error::Error + Send + Sync>> {
    Ok(InvitedRoomUpdate { invite_state: None })
}

async fn build_initial_left_room_update_ws(
    state: &AppState,
    room_id: &str,
) -> Result<LeftRoomUpdate, Box<dyn std::error::Error + Send + Sync>> {
    Ok(LeftRoomUpdate { timeline: None, state: None })
}
