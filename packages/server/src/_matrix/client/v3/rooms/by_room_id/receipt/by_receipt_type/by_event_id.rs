use crate::{AppState, auth::AuthenticatedUser, federation::outbound_queue::OutboundEvent};
use axum::{Extension, Json, extract::Path, http::StatusCode};
use matryx_entity::types::{EDU, EphemeralEvent, EventContent};
use matryx_surrealdb::repository::{MembershipRepository, ReceiptRepository};
use serde_json::{Value, json};
use tracing::{debug, error, info, warn};

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

            // Federate public read receipts to remote servers
            let membership_repo = MembershipRepository::new(state.db.clone());
            let remote_servers = match membership_repo.get_remote_servers_in_room(&room_id).await {
                Ok(servers) => servers,
                Err(e) => {
                    error!("Failed to get remote servers for room {}: {}", room_id, e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                },
            };

            // Filter out our own server
            let remote_servers: Vec<String> = remote_servers
                .into_iter()
                .filter(|server| server != &state.homeserver_name)
                .collect();

            if !remote_servers.is_empty() {
                // Build receipt content according to Matrix spec
                let mut receipt_data = json!({
                    event_id.clone(): {
                        "m.read": {
                            user.user_id.clone(): {
                                "ts": chrono::Utc::now().timestamp_millis()
                            }
                        }
                    }
                });

                // Add thread_id if present (Matrix 1.4 threading)
                if let Some(thread) = &thread_id
                    && let Some(event_obj) = receipt_data.get_mut(&event_id)
                    && let Some(read_obj) = event_obj.get_mut("m.read")
                    && let Some(user_obj) = read_obj.get_mut(&user.user_id)
                {
                    if let Some(user_obj_map) = user_obj.as_object_mut() {
                        user_obj_map.insert("thread_id".to_string(), json!(thread));
                    } else {
                        error!("Receipt user object is not a JSON object for user {}", user.user_id);
                    }
                }

                let receipt_content = json!({
                    "room_id": room_id,
                    "type": "m.receipt",
                    "content": receipt_data,
                });

                let ephemeral_event = EphemeralEvent::new(
                    EventContent::Unknown(receipt_content),
                    "m.receipt".to_string(),
                    Some(room_id.clone()),
                    user.user_id.clone(),
                );

                let edu = EDU::new(ephemeral_event, true);

                for destination in remote_servers {
                    let event = OutboundEvent::Edu {
                        destination: destination.clone(),
                        edu: Box::new(edu.clone()),
                    };

                    if let Err(e) = state.outbound_tx.send(event) {
                        error!("Failed to queue receipt EDU to {}: {}", destination, e);
                    } else {
                        debug!(
                            "Queued receipt EDU to {} for user {} in room {}",
                            destination, user.user_id, room_id
                        );
                    }
                }
            }
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
