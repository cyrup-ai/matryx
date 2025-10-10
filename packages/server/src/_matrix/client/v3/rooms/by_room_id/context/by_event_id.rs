use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::{error, info};

use crate::{
    auth::{MatrixAuthError, extract_matrix_auth},
    state::AppState,
};

#[derive(Deserialize)]
pub struct ContextParams {
    pub limit: Option<u32>,
    pub filter: Option<String>,
}

/// GET /_matrix/client/v3/rooms/{roomId}/context/{eventId}
pub async fn get(
    Path((room_id, event_id)): Path<(String, String)>,
    Query(params): Query<ContextParams>,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    info!("Event context requested for event {} in room {}", event_id, room_id);

    // Extract user authentication
    let matrix_auth =
        extract_matrix_auth(&headers, &state.session_service)
            .await
            .map_err(|e| match e {
                MatrixAuthError::MissingToken | MatrixAuthError::MissingAuthorization => {
                    StatusCode::UNAUTHORIZED
                },
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            })?;

    let (user_id, is_guest) = match matrix_auth {
        crate::auth::MatrixAuth::User(user_auth) => {
            // Get session to check if user is a guest
            let session_repo = matryx_surrealdb::repository::SessionRepository::new(state.db.clone());
            let session = session_repo
                .get_by_access_token(&user_auth.token)
                .await
                .map_err(|e| {
                    error!("Failed to get session: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;
            
            let is_guest = session.map(|s| s.is_guest).unwrap_or(false);
            (user_auth.user_id, is_guest)
        },
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    // Check guest access before retrieving context
    let room_repo = matryx_surrealdb::repository::RoomRepository::new(state.db.clone());
    crate::room::authorization::require_room_access(&room_repo, &room_id, &user_id, is_guest)
        .await?;

    // Use RoomOperationsService to get event context with permission validation
    let limit = params.limit.unwrap_or(10).min(100); // Cap at 100 events

    // Process filter parameter if provided (Matrix spec: filter ID for lazy loading and event filtering)
    let matrix_filter = if let Some(filter_id) = params.filter.as_ref() {
        // Resolve filter from filter repository per Matrix specification
        let filter_repo =
            matryx_surrealdb::repository::filter::FilterRepository::new(state.db.clone());
        match filter_repo.get_by_id(filter_id).await {
            Ok(Some(filter)) => {
                info!(
                    "Applied filter {} to context request for event {} in room {}",
                    filter_id, event_id, room_id
                );
                Some(filter)
            },
            Ok(None) => {
                error!("Filter {} not found for context request", filter_id);
                return Err(StatusCode::BAD_REQUEST);
            },
            Err(e) => {
                error!("Failed to resolve filter {} for context request: {}", filter_id, e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            },
        }
    } else {
        None
    };

    match state
        .room_operations
        .get_event_context(&room_id, &event_id, limit, &user_id)
        .await
    {
        Ok(mut context_response) => {
            info!("Successfully retrieved context for event {} in room {}", event_id, room_id);

            // Apply filter to context response if provided (Matrix spec compliance)
            if let Some(ref filter) = matrix_filter {
                // Apply room event filter to events_before, events_after, and state
                if let Some(ref room_filter) = filter.room {
                    if let Some(ref timeline_filter) = room_filter.timeline {
                        // Apply filter to events_before and events_after
                        let events_before_json: Vec<serde_json::Value> = context_response
                            .events_before
                            .into_iter()
                            .map(|e| serde_json::to_value(e).unwrap_or_default())
                            .collect();
                        let filtered_events_before =
                            apply_context_event_filter(events_before_json, &timeline_filter.base);
                        context_response.events_before = filtered_events_before
                            .into_iter()
                            .map(|v| serde_json::from_value(v).unwrap_or_default())
                            .collect();

                        let events_after_json: Vec<serde_json::Value> = context_response
                            .events_after
                            .into_iter()
                            .map(|e| serde_json::to_value(e).unwrap_or_default())
                            .collect();
                        let filtered_events_after =
                            apply_context_event_filter(events_after_json, &timeline_filter.base);
                        context_response.events_after = filtered_events_after
                            .into_iter()
                            .map(|v| serde_json::from_value(v).unwrap_or_default())
                            .collect();
                    }

                    if let Some(ref state_filter) = room_filter.state {
                        // Apply filter to state events
                        let state_json: Vec<serde_json::Value> = context_response
                            .state
                            .into_iter()
                            .map(|e| serde_json::to_value(e).unwrap_or_default())
                            .collect();
                        let filtered_state =
                            apply_context_event_filter(state_json, &state_filter.base);
                        context_response.state = filtered_state
                            .into_iter()
                            .map(|v| serde_json::from_value(v).unwrap_or_default())
                            .collect();
                    }
                }

                // Apply event_fields filtering if specified
                if let Some(ref event_fields) = filter.event_fields {
                    let events_before_json: Vec<serde_json::Value> = context_response
                        .events_before
                        .into_iter()
                        .map(|e| serde_json::to_value(e).unwrap_or_default())
                        .collect();
                    let filtered_events_before =
                        self::apply_event_fields_filter(events_before_json, event_fields);
                    context_response.events_before = filtered_events_before
                        .into_iter()
                        .map(|v| serde_json::from_value(v).unwrap_or_default())
                        .collect();

                    let events_after_json: Vec<serde_json::Value> = context_response
                        .events_after
                        .into_iter()
                        .map(|e| serde_json::to_value(e).unwrap_or_default())
                        .collect();
                    let filtered_events_after =
                        self::apply_event_fields_filter(events_after_json, event_fields);
                    context_response.events_after = filtered_events_after
                        .into_iter()
                        .map(|v| serde_json::from_value(v).unwrap_or_default())
                        .collect();

                    let state_json: Vec<serde_json::Value> = context_response
                        .state
                        .into_iter()
                        .map(|e| serde_json::to_value(e).unwrap_or_default())
                        .collect();
                    let filtered_state = self::apply_event_fields_filter(state_json, event_fields);
                    context_response.state = filtered_state
                        .into_iter()
                        .map(|v| serde_json::from_value(v).unwrap_or_default())
                        .collect();

                    // Apply to the main event as well
                    if let Some(ref mut event) = context_response.event {
                        let event_json = serde_json::to_value(event.clone()).unwrap_or_default();
                        let filtered_event_json =
                            self::apply_event_fields_filter_single(event_json, event_fields);
                        *event = serde_json::from_value(filtered_event_json).unwrap_or_default();
                    }
                }
            }

            // Convert ContextResponse to Matrix API format
            Ok(Json(json!({
                "start": context_response.start,
                "end": context_response.end,
                "events_before": context_response.events_before,
                "event": context_response.event,
                "events_after": context_response.events_after,
                "state": context_response.state
            })))
        },
        Err(e) => {
            error!("Failed to get event context for event {} in room {}: {}", event_id, room_id, e);
            match e {
                matryx_surrealdb::repository::error::RepositoryError::NotFound { .. } => {
                    Err(StatusCode::NOT_FOUND)
                },
                matryx_surrealdb::repository::error::RepositoryError::Unauthorized { .. } => {
                    Err(StatusCode::FORBIDDEN)
                },
                _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        },
    }
}

/// Apply EventFilter to a list of events per Matrix specification
/// Filters by event types, senders, and other EventFilter criteria
fn apply_context_event_filter(
    events: Vec<serde_json::Value>,
    event_filter: &matryx_entity::filter::EventFilter,
) -> Vec<serde_json::Value> {
    events
        .into_iter()
        .filter(|event| {
            // Apply event type filtering
            if let Some(ref types) = event_filter.types
                && !types.is_empty()
                && !types.contains(&"*".to_string())
            {
                let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let matches_type = types.iter().any(|filter_type| {
                    if filter_type.ends_with('*') {
                        let prefix = &filter_type[..filter_type.len() - 1];
                        event_type.starts_with(prefix)
                    } else {
                        event_type == filter_type
                    }
                });
                if !matches_type {
                    return false;
                }
            }

            // Apply not_types filtering
            if let Some(ref not_types) = event_filter.not_types {
                let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let matches_excluded = not_types.iter().any(|filter_type| {
                    if filter_type.ends_with('*') {
                        let prefix = &filter_type[..filter_type.len() - 1];
                        event_type.starts_with(prefix)
                    } else {
                        event_type == filter_type
                    }
                });
                if matches_excluded {
                    return false;
                }
            }

            // Apply sender filtering
            if let Some(ref senders) = event_filter.senders
                && !senders.is_empty()
            {
                let sender = event.get("sender").and_then(|s| s.as_str()).unwrap_or("");
                if !senders.contains(&sender.to_string()) {
                    return false;
                }
            }

            // Apply not_senders filtering
            if let Some(ref not_senders) = event_filter.not_senders {
                let sender = event.get("sender").and_then(|s| s.as_str()).unwrap_or("");
                if not_senders.contains(&sender.to_string()) {
                    return false;
                }
            }

            true
        })
        .collect()
}

/// Apply event_fields filtering to a list of events per Matrix specification
/// Only includes specified dot-separated field paths in events
fn apply_event_fields_filter(
    events: Vec<serde_json::Value>,
    event_fields: &[String],
) -> Vec<serde_json::Value> {
    events
        .into_iter()
        .map(|event| apply_event_fields_filter_single(event, event_fields))
        .collect()
}

/// Apply event_fields filtering to a single event per Matrix specification
/// Only includes specified dot-separated field paths in the event
fn apply_event_fields_filter_single(
    mut event: serde_json::Value,
    event_fields: &[String],
) -> serde_json::Value {
    if let serde_json::Value::Object(ref mut event_obj) = event {
        let original_event = event_obj.clone();
        event_obj.clear();

        // Include only specified fields
        for field_path in event_fields {
            let path_parts: Vec<&str> = field_path.split('.').collect();
            if let Some(value) =
                get_nested_value(&serde_json::Value::Object(original_event.clone()), &path_parts)
            {
                set_nested_value(event_obj, &path_parts, value);
            }
        }
    }
    event
}

/// Get nested value from JSON object using dot-separated path
fn get_nested_value(value: &serde_json::Value, path: &[&str]) -> Option<serde_json::Value> {
    if path.is_empty() {
        return Some(value.clone());
    }

    let key = path[0];
    let remaining_path = &path[1..];

    match value {
        serde_json::Value::Object(obj) => {
            obj.get(key).and_then(|nested| get_nested_value(nested, remaining_path))
        },
        _ => None,
    }
}

/// Set nested value in JSON object using dot-separated path
fn set_nested_value(
    obj: &mut serde_json::Map<String, serde_json::Value>,
    path: &[&str],
    value: serde_json::Value,
) {
    if path.is_empty() {
        return;
    }

    let key = path[0];
    let remaining_path = &path[1..];

    if remaining_path.is_empty() {
        obj.insert(key.to_string(), value);
    } else {
        let nested_obj = obj
            .entry(key.to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

        if let serde_json::Value::Object(nested_map) = nested_obj {
            set_nested_value(nested_map, remaining_path, value);
        }
    }
}
