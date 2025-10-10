use crate::{AppState, auth::AuthenticatedUser, federation::outbound_queue::OutboundEvent};
use axum::{Extension, Json, extract::Path, http::StatusCode};
use matryx_entity::types::{EDU, EphemeralEvent, EventContent};
use matryx_surrealdb::repository::{FederationRepository, MembershipRepository};
use serde_json::{Value, json};
use tracing::{debug, error};

/// PUT /_matrix/client/v3/rooms/{roomId}/typing/{userId}
pub async fn put(
    Path((room_id, user_id)): Path<(String, String)>,
    Extension(state): Extension<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // Verify user is acting on their own behalf
    if auth_user.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Extract typing state and timeout
    let typing = payload.get("typing").and_then(|v| v.as_bool()).unwrap_or(false);

    // Store typing state locally first
    let federation_repo = FederationRepository::new(state.db.clone());
    let server_name = state.homeserver_name.as_str();

    if let Err(e) = federation_repo
        .process_typing_edu(&room_id, &user_id, server_name, typing)
        .await
    {
        error!("Failed to store local typing state: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Get remote servers in the room for federation
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

    // Send typing EDU to each remote server
    if !remote_servers.is_empty() {
        let typing_content = json!({
            "room_id": room_id,
            "user_id": user_id,
            "typing": typing,
        });

        let ephemeral_event = EphemeralEvent::new(
            EventContent::Unknown(typing_content),
            "m.typing".to_string(),
            Some(room_id.clone()),
            user_id.clone(),
        );

        let edu = EDU::new(ephemeral_event, true);

        for destination in remote_servers {
            let event = OutboundEvent::Edu {
                destination: destination.clone(),
                edu: Box::new(edu.clone()),
            };

            if let Err(e) = state.outbound_tx.send(event) {
                error!("Failed to queue typing EDU to {}: {}", destination, e);
            } else {
                debug!(
                    "Queued typing EDU to {} for user {} in room {}",
                    destination, user_id, room_id
                );
            }
        }
    }

    Ok(Json(json!({})))
}
