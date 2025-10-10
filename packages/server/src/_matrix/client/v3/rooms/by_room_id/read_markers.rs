use axum::{Json, extract::{Path, State}, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, error, info, warn};

use crate::{AppState, auth::{MatrixAuth, extract_matrix_auth}};
use matryx_surrealdb::repository::ReceiptRepository;

#[derive(Debug, Deserialize)]
pub struct ReadMarkersRequest {
    #[serde(rename = "m.fully_read")]
    pub fully_read: Option<String>,
    #[serde(rename = "m.read")]
    pub read: Option<String>,
    #[serde(rename = "m.read.private")]
    pub read_private: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReadMarkersResponse {}

/// POST /_matrix/client/v3/rooms/{roomId}/read_markers
/// 
/// Set the position of the read marker for a given room, and optionally
/// the read receipt's location.
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Json(payload): Json<ReadMarkersRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service).await.map_err(|e| {
        warn!("Read markers update failed - authentication extraction failed: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    let user_id = match auth {
        MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                warn!("Read markers update failed - access token expired");
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id.clone()
        },
        MatrixAuth::Server(_) => {
            warn!("Read markers update failed - server authentication not allowed");
            return Err(StatusCode::FORBIDDEN);
        },
        MatrixAuth::Anonymous => {
            warn!("Read markers update failed - anonymous authentication not allowed");
            return Err(StatusCode::UNAUTHORIZED);
        },
    };

    info!("Processing read markers for room: {} user: {}", room_id, user_id);

    // Update fully read marker if provided
    if let Some(ref event_id) = payload.fully_read {
        match state.room_operations.event_repo().mark_event_as_read(&room_id, event_id, &user_id).await {
            Ok(()) => {
                info!("Updated fully read marker to {} for user {} in room {}", event_id, user_id, room_id);
            },
            Err(e) => {
                error!("Failed to update fully read marker: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // Process read receipts as required by Matrix 1.4 specification
    // The server MUST treat m.read and m.read.private in /read_markers the same
    // as it would for requests to /receipt/{receiptType}/{eventId}
    let receipt_repo = ReceiptRepository::new(state.db.clone());

    // Process m.read (public read receipt)
    if let Some(ref event_id) = payload.read {
        match receipt_repo
            .store_receipt(
                &room_id,
                &user_id,
                event_id,
                "m.read",
                None,  // read_markers payload doesn't include thread_id
                &state.homeserver_name,
            )
            .await
        {
            Ok(()) => {
                debug!("Stored m.read receipt for user {} in room {} at event {}", user_id, room_id, event_id);
            },
            Err(e) => {
                error!("Failed to store m.read receipt: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    // Process m.read.private (private read receipt)
    if let Some(ref event_id) = payload.read_private {
        match receipt_repo
            .store_receipt(
                &room_id,
                &user_id,
                event_id,
                "m.read.private",
                None,  // read_markers payload doesn't include thread_id
                &state.homeserver_name,
            )
            .await
        {
            Ok(()) => {
                debug!("Stored m.read.private receipt for user {} in room {} at event {}", user_id, room_id, event_id);
            },
            Err(e) => {
                error!("Failed to store m.read.private receipt: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
    }

    Ok(Json(json!({})))
}
