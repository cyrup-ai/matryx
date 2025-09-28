use crate::{AppState, auth::AuthenticatedUser};
use axum::{Extension, Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};
use tracing::{error, info, warn};
use matryx_surrealdb::repository::ReceiptRepository;

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
    
    // Initialize receipt repository
    let receipt_repo = ReceiptRepository::new(state.db.clone());

    match receipt_type.as_str() {
        "m.read" => {
            // Public read receipt
            if let Err(e) = receipt_repo
                .store_receipt(
                    &room_id,
                    &user.user_id,
                    &event_id,
                    "m.read",
                    thread_id.as_deref(),
                    &state.homeserver_name,
                )
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
            if let Err(e) = receipt_repo
                .store_receipt(
                    &room_id,
                    &user.user_id,
                    &event_id,
                    "m.read.private",
                    thread_id.as_deref(),
                    &state.homeserver_name,
                )
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
