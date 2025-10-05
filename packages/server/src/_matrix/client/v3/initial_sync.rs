use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{error, info};

use crate::auth::extract_matrix_auth;
use crate::state::AppState;
use matryx_surrealdb::repository::sync::{Filter, RoomEventFilter, RoomFilter, SyncRepository};

#[derive(Deserialize)]
pub struct InitialSyncQuery {
    limit: Option<u32>,
    archived: Option<bool>,
}

/// GET /_matrix/client/v3/initial_sync
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<InitialSyncQuery>,
) -> Result<Json<Value>, StatusCode> {
    // Extract and validate Matrix authentication
    let auth = extract_matrix_auth(&headers, &state.session_service)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user_id = match auth {
        crate::auth::MatrixAuth::User(token_info) => {
            if token_info.is_expired() {
                return Err(StatusCode::UNAUTHORIZED);
            }
            token_info.user_id
        },
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    info!("Getting initial sync for user: {}", user_id);

    // Create sync repository
    let sync_repo = SyncRepository::new(state.db.clone());

    // Apply query parameters for pagination and filtering
    if query.archived.unwrap_or(false) {
        tracing::info!("Initial sync requested archived rooms for user: {}", user_id);
    }

    // Create filter if limit is specified
    let filter = query.limit.map(|limit| Filter {
        room: Some(RoomFilter {
            timeline: Some(RoomEventFilter {
                limit: Some(limit as u64),
                not_senders: None,
                not_types: None,
                senders: None,
                types: None,
                contains_url: None,
                lazy_load_members: None,
                include_redundant_members: None,
                not_rooms: None,
                rooms: None,
                unread_thread_notifications: None,
            }),
            not_rooms: None,
            rooms: None,
            ephemeral: None,
            include_leave: None,
            state: None,
            account_data: None,
        }),
        presence: None,
        account_data: None,
        event_format: None,
        event_fields: None,
    });

    // Get initial sync data
    let initial_sync = match sync_repo.get_initial_sync_data(&user_id, filter.as_ref()).await {
        Ok(sync_data) => sync_data,
        Err(e) => {
            error!("Failed to get initial sync for user {}: {}", user_id, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Convert to Matrix API format
    let rooms_json: Vec<Value> = initial_sync
        .rooms
        .into_iter()
        .map(|room| {
            json!({
                "room_id": room.room_id,
                "state": room.state,
                "timeline": {
                    "events": room.timeline.events,
                    "limited": room.timeline.limited,
                    "prev_batch": room.timeline.prev_batch
                },
                "ephemeral": {
                    "events": room.ephemeral.events
                },
                "account_data": room.account_data,
                "unread_notifications": {
                    "highlight_count": room.unread_notifications.highlight_count,
                    "notification_count": room.unread_notifications.notification_count
                },
                "summary": room.summary
            })
        })
        .collect();

    Ok(Json(json!({
        "rooms": rooms_json,
        "presence": initial_sync.presence.events,
        "account_data": initial_sync.account_data.events,
        "next_batch": initial_sync.next_batch
    })))
}
