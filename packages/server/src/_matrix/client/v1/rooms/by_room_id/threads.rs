use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

use crate::{
    auth::MatrixSessionService,
    database::SurrealRepository,
    AppState,
};

#[derive(Deserialize)]
pub struct ThreadsQuery {
    pub include: Option<String>, // "all" or "participated"
    pub from: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct Event {
    pub content: Value,
    pub event_id: String,
    pub origin_server_ts: u64,
    pub room_id: String,
    pub sender: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub unsigned: Option<Value>,
}

#[derive(Serialize, Deserialize)]
pub struct ThreadSummary {
    pub latest_event: Event,
    pub count: u64,
    pub current_user_participated: bool,
}

#[derive(Serialize)]
pub struct ThreadsResponse {
    pub chunk: Vec<ThreadSummary>,
    pub next_token: Option<String>,
    pub prev_token: Option<String>,
}

/// GET /_matrix/client/v1/rooms/{roomId}/threads
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(room_id): Path<String>,
    Query(query): Query<ThreadsQuery>,
) -> Result<Json<ThreadsResponse>, StatusCode> {
    // Extract and validate access token
    let access_token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token and get user context
    let token_info = state.session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Verify user is member of the room
    let membership_query = "SELECT membership FROM room_members WHERE room_id = $room_id AND user_id = $user_id";
    let mut membership_params = HashMap::new();
    membership_params.insert("room_id".to_string(), Value::String(room_id.clone()));
    membership_params.insert("user_id".to_string(), Value::String(token_info.user_id.clone()));

    let membership_result = state.database
        .query(membership_query, Some(membership_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let is_member = membership_result
        .first()
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("membership"))
        .and_then(|v| v.as_str())
        .map(|membership| membership == "join" || membership == "invite")
        .unwrap_or(false);

    if !is_member {
        return Err(StatusCode::FORBIDDEN);
    }

    // Set defaults
    let include = query.include.as_deref().unwrap_or("all");
    let limit = query.limit.unwrap_or(20).min(100);

    // Get thread summaries
    let threads = get_room_threads(
        &state,
        &room_id,
        &token_info.user_id,
        include,
        limit,
    ).await?;

    Ok(Json(ThreadsResponse {
        chunk: threads,
        next_token: None, // TODO: Implement pagination
        prev_token: None,
    }))
}

async fn get_room_threads(
    state: &AppState,
    room_id: &str,
    user_id: &str,
    include: &str,
    limit: u32,
) -> Result<Vec<ThreadSummary>, StatusCode> {
    // Find thread root events (events that have replies with m.relates_to.rel_type = "m.thread")
    let thread_roots_query = r#"
        SELECT DISTINCT thread_root_id, 
               (SELECT * FROM events WHERE event_id = $parent.thread_root_id AND room_id = $room_id)[0] as root_event,
               count() as thread_count,
               max(created_at) as latest_activity
        FROM event_threads 
        WHERE room_id = $room_id
        GROUP BY thread_root_id
        ORDER BY latest_activity DESC
        LIMIT $limit
    "#;

    let mut params = HashMap::new();
    params.insert("room_id".to_string(), Value::String(room_id.to_string()));
    params.insert("limit".to_string(), Value::Number(serde_json::Number::from(limit)));

    let result = state.database
        .query(thread_roots_query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut threads = Vec::new();

    if let Some(thread_rows) = result.first() {
        for thread_row in thread_rows {
            if let (Some(thread_root_id), Some(root_event_data), Some(thread_count)) = (
                thread_row.get("thread_root_id").and_then(|v| v.as_str()),
                thread_row.get("root_event"),
                thread_row.get("thread_count").and_then(|v| v.as_u64()),
            ) {
                // Check if user participated in thread (if include = "participated")
                let user_participated = if include == "participated" {
                    check_user_thread_participation(state, thread_root_id, user_id).await?
                } else {
                    true // Include all threads
                };

                if !user_participated && include == "participated" {
                    continue;
                }

                // Get latest event in thread
                let latest_event = get_latest_thread_event(state, thread_root_id, room_id).await?;

                if let Some(latest) = latest_event {
                    let thread_summary = ThreadSummary {
                        latest_event: latest,
                        count: thread_count,
                        current_user_participated: check_user_thread_participation(state, thread_root_id, user_id).await?,
                    };

                    threads.push(thread_summary);
                }
            }
        }
    }

    Ok(threads)
}

async fn check_user_thread_participation(
    state: &AppState,
    thread_root_id: &str,
    user_id: &str,
) -> Result<bool, StatusCode> {
    let query = r#"
        SELECT count() as participation_count
        FROM events e
        JOIN event_threads et ON e.event_id = et.event_id
        WHERE et.thread_root_id = $thread_root_id AND e.sender = $user_id
    "#;

    let mut params = HashMap::new();
    params.insert("thread_root_id".to_string(), Value::String(thread_root_id.to_string()));
    params.insert("user_id".to_string(), Value::String(user_id.to_string()));

    let result = state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let participated = result
        .first()
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("participation_count"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) > 0;

    Ok(participated)
}

async fn get_latest_thread_event(
    state: &AppState,
    thread_root_id: &str,
    room_id: &str,
) -> Result<Option<Event>, StatusCode> {
    let query = r#"
        SELECT e.*
        FROM events e
        JOIN event_threads et ON e.event_id = et.event_id
        WHERE et.thread_root_id = $thread_root_id AND e.room_id = $room_id
        ORDER BY e.origin_server_ts DESC
        LIMIT 1
    "#;

    let mut params = HashMap::new();
    params.insert("thread_root_id".to_string(), Value::String(thread_root_id.to_string()));
    params.insert("room_id".to_string(), Value::String(room_id.to_string()));

    let result = state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(event_data) = result.first().and_then(|rows| rows.first()) {
        let event = Event {
            content: event_data.get("content").cloned().unwrap_or(Value::Object(serde_json::Map::new())),
            event_id: event_data.get("event_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            origin_server_ts: event_data.get("origin_server_ts").and_then(|v| v.as_u64()).unwrap_or(0),
            room_id: event_data.get("room_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            sender: event_data.get("sender").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            event_type: event_data.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            unsigned: event_data.get("unsigned").cloned(),
        };

        Ok(Some(event))
    } else {
        Ok(None)
    }
}