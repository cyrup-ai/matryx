use crate::{AppState, auth::AuthenticatedUser};
use axum::{Extension, Json, extract::Path, http::StatusCode};
use chrono::Utc;
use serde_json::{Value, json};
use tracing::{error, info, warn};

/// POST /_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}
///
/// Matrix 1.4 specification compliant receipt handling with threading support
pub async fn post(
    Path((room_id, receipt_type, event_id)): Path<(String, String, String)>,
    Extension(state): Extension<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Extract threading information from payload (Matrix 1.4 requirement)
    let thread_id = payload.get("thread_id").and_then(|v| v.as_str()).map(|s| s.to_string());

    match receipt_type.as_str() {
        "m.read" => {
            // Public read receipt
            let query = "
                INSERT INTO receipts SET
                    room_id = $room_id,
                    user_id = $user_id,
                    event_id = $event_id,
                    receipt_type = 'm.read',
                    thread_id = $thread_id,
                    timestamp = $timestamp,
                    is_private = false,
                    server_name = $server_name,
                    received_at = $received_at
                ON DUPLICATE KEY UPDATE
                    event_id = $event_id,
                    timestamp = $timestamp,
                    thread_id = $thread_id,
                    received_at = $received_at
            ";

            if let Err(e) = state
                .db
                .query(query)
                .bind(("room_id", room_id.clone()))
                .bind(("user_id", user.user_id.clone()))
                .bind(("event_id", event_id.clone()))
                .bind(("thread_id", thread_id.clone()))
                .bind(("timestamp", Utc::now().timestamp_millis()))
                .bind(("server_name", state.homeserver_name.clone()))
                .bind(("received_at", Utc::now()))
                .await
            {
                error!("Failed to store m.read receipt: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }

            info!(
                "Processed m.read receipt: user={}, room={}, event={}, thread={:?}",
                user.user_id, room_id, event_id, thread_id
            );
        },

        "m.read.private" => {
            // Private read receipt - Matrix 1.4 specification
            let query = "
                INSERT INTO receipts SET
                    room_id = $room_id,
                    user_id = $user_id,
                    event_id = $event_id,
                    receipt_type = 'm.read.private',
                    thread_id = $thread_id,
                    timestamp = $timestamp,
                    is_private = true,
                    server_name = $server_name,
                    received_at = $received_at
                ON DUPLICATE KEY UPDATE
                    event_id = $event_id,
                    timestamp = $timestamp,
                    thread_id = $thread_id,
                    received_at = $received_at
            ";

            if let Err(e) = state
                .db
                .query(query)
                .bind(("room_id", room_id.clone()))
                .bind(("user_id", user.user_id.clone()))
                .bind(("event_id", event_id.clone()))
                .bind(("thread_id", thread_id.clone()))
                .bind(("timestamp", Utc::now().timestamp_millis()))
                .bind(("server_name", state.homeserver_name.clone()))
                .bind(("received_at", Utc::now()))
                .await
            {
                error!("Failed to store m.read.private receipt: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }

            info!(
                "Processed m.read.private receipt: user={}, room={}, event={}, thread={:?}",
                user.user_id, room_id, event_id, thread_id
            );

            // CRITICAL: Private receipts are NEVER federated per Matrix specification
        },

        _ => {
            warn!("Unsupported receipt type '{}' from user {}", receipt_type, user.user_id);
            return Err(StatusCode::BAD_REQUEST);
        },
    }

    Ok(Json(json!({})))
}
