use axum::{
    extract::{
        Query,
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::Response,
};
use chrono::Utc;
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use matryx_surrealdb::repository::InfrastructureService;

use surrealdb::engine::any::Any;

#[derive(Deserialize)]
pub struct SyncWebSocketQuery {
    filter: Option<String>,
    timeout: Option<u64>,
    set_presence: Option<String>,
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

/// WebSocket endpoint for Matrix sync using InfrastructureService
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
    let device_id = if auth.device_id.is_empty() {
        format!("WS_{}", Uuid::new_v4())
    } else {
        auth.device_id.clone()
    };
    let connection_id = format!("ws_{}_{}", user_id, Uuid::new_v4());

    info!("WebSocket connection established for user: {} device: {}", user_id, device_id);

    // Create InfrastructureService instance
    let infrastructure_service = create_infrastructure_service(&state).await;

    // Register the WebSocket connection
    if let Err(e) = infrastructure_service
        .register_websocket_connection(&user_id, &device_id, &connection_id)
        .await
    {
        error!("Failed to register WebSocket connection: {:?}", e);
        return;
    }

    let (sender, receiver) = StreamExt::split(socket);
    let (tx, rx) = mpsc::channel::<Message>(100);

    // Spawn task to handle outgoing messages
    let send_task = tokio::spawn(handle_outgoing_messages(sender, rx));

    // Spawn task to handle incoming messages (ping/pong, control messages)
    let recv_task = tokio::spawn(handle_incoming_messages(receiver, tx.clone()));

    // Spawn task to handle sync using InfrastructureService
    let sync_task = tokio::spawn(handle_sync_with_service(
        infrastructure_service,
        user_id.clone(),
        device_id.clone(),
        tx.clone(),
        query,
    ));

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

async fn create_infrastructure_service(state: &AppState) -> InfrastructureService<Any> {
    let websocket_repo = matryx_surrealdb::repository::WebSocketRepository::new(state.db.clone());
    let transaction_repo =
        matryx_surrealdb::repository::TransactionRepository::new(state.db.clone());
    let key_server_repo = matryx_surrealdb::repository::KeyServerRepository::new(state.db.clone());
    let registration_repo =
        matryx_surrealdb::repository::RegistrationRepository::new(state.db.clone());
    let directory_repo = matryx_surrealdb::repository::DirectoryRepository::new(state.db.clone());
    let device_repo = matryx_surrealdb::repository::DeviceRepository::new(state.db.clone());

    InfrastructureService::new(
        websocket_repo,
        transaction_repo,
        key_server_repo,
        registration_repo,
        directory_repo,
        device_repo,
    )
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
    use futures::StreamExt;

    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Handle JSON control messages
                if let Ok(pong) = serde_json::from_str::<PongMessage>(&text) {
                    if pong.message_type == "pong" {
                        debug!("Received pong from client at {}", pong.timestamp);
                    }
                } else {
                    // Forward non-control messages to the message handler
                    if let Err(e) = tx.send(Message::Text(text)).await {
                        error!("Failed to forward WebSocket message: {}", e);
                        break;
                    }
                }
            },
            Ok(Message::Pong(data)) => {
                debug!("Received WebSocket pong");
                // Forward pong messages to maintain the protocol
                if let Err(e) = tx.send(Message::Pong(data)).await {
                    error!("Failed to forward WebSocket pong: {}", e);
                    break;
                }
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

async fn handle_sync_with_service(
    infrastructure_service: InfrastructureService<Any>,
    user_id: String,
    device_id: String,
    tx: mpsc::Sender<Message>,
    query: SyncWebSocketQuery,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Apply sync filter if provided
    let sync_filter = query.filter.as_deref();
    debug!("WebSocket sync with filter: {:?}", sync_filter);

    // Set user presence if specified (online, offline, unavailable)
    if let Some(presence) = &query.set_presence {
        if matches!(presence.as_str(), "online" | "offline" | "unavailable") {
            info!("Setting user {} presence to: {}", user_id, presence);
            // TODO: Implement presence setting via infrastructure service
            debug!("Presence setting not yet implemented: {}", presence);
        } else {
            warn!("Invalid presence value: {}", presence);
        }
    }

    // Send initial sync using InfrastructureService with filter
    match infrastructure_service
        .handle_websocket_sync(&user_id, &device_id, sync_filter)
        .await
    {
        Ok(sync_response) => {
            let ws_message = WebSocketMessage {
                message_type: "initial_sync".to_string(),
                data: serde_json::to_value(&sync_response)?,
            };

            let message_json = serde_json::to_string(&ws_message)?;
            let message = Message::Text(message_json.into());

            if tx.send(message).await.is_err() {
                error!("Failed to send initial sync");
                return Ok(());
            }
        },
        Err(e) => {
            error!("Failed to get initial sync: {:?}", e);
            let error_message = WebSocketMessage {
                message_type: "error".to_string(),
                data: json!({ "error": format!("Sync error: {}", e) }),
            };

            let message_json = serde_json::to_string(&error_message)?;
            let message = Message::Text(message_json.into());

            if tx.send(message).await.is_err() {
                return Ok(());
            }
        },
    }

    // Set up periodic sync updates using InfrastructureService
    let mut sync_interval = interval(Duration::from_secs(query.timeout.unwrap_or(30)));
    let mut last_batch: Option<String> = None;

    loop {
        sync_interval.tick().await;

        match infrastructure_service
            .handle_websocket_sync(&user_id, &device_id, last_batch.as_deref())
            .await
        {
            Ok(sync_response) => {
                // Only send update if there are changes
                let has_changes = !sync_response.rooms.join.is_empty() ||
                    !sync_response.rooms.invite.is_empty() ||
                    !sync_response.rooms.leave.is_empty();

                if has_changes {
                    last_batch = Some(sync_response.next_batch.clone());

                    let ws_message = WebSocketMessage {
                        message_type: "sync".to_string(),
                        data: serde_json::to_value(&sync_response)?,
                    };

                    let message_json = serde_json::to_string(&ws_message)?;
                    let message = Message::Text(message_json.into());

                    if tx.send(message).await.is_err() {
                        break;
                    }
                }
            },
            Err(e) => {
                warn!("Sync error: {:?}", e);
                let error_message = WebSocketMessage {
                    message_type: "error".to_string(),
                    data: json!({ "error": format!("Sync error: {}", e) }),
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
