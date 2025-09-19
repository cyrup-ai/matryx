use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::{
    auth::MatrixSessionService,
    database::SurrealRepository,
    AppState,
};

#[derive(Deserialize)]
pub struct RelationsQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<u32>,
    pub dir: Option<String>, // "f" or "b"
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
pub struct ReactionAggregation {
    pub key: String,
    pub count: u64,
    pub users: Vec<String>,
}

#[derive(Serialize)]
pub struct RelationsResponse {
    pub chunk: Vec<Event>,
    pub aggregations: HashMap<String, ReactionAggregation>,
    pub next_batch: Option<String>,
    pub prev_batch: Option<String>,
}

/// GET /_matrix/client/v1/rooms/{roomId}/relations/{eventId}
pub async fn get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((room_id, event_id)): Path<(String, String)>,
    Query(query): Query<RelationsQuery>,
) -> Result<Json<RelationsResponse>, StatusCode> {
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

    // Verify the target event exists and user can access it
    let event_exists_query = "SELECT event_id FROM events WHERE event_id = $event_id AND room_id = $room_id";
    let mut event_params = HashMap::new();
    event_params.insert("event_id".to_string(), Value::String(event_id.clone()));
    event_params.insert("room_id".to_string(), Value::String(room_id.clone()));

    let event_result = state.database
        .query(event_exists_query, Some(event_params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let event_exists = event_result
        .first()
        .and_then(|rows| rows.first())
        .is_some();

    if !event_exists {
        return Err(StatusCode::NOT_FOUND);
    }

    // Set defaults
    let limit = query.limit.unwrap_or(50).min(100);

    // Get related events
    let relations = get_event_relations(
        &state,
        &room_id,
        &event_id,
        None, // rel_type
        None, // event_type
        limit,
    ).await?;

    // Get reaction aggregations
    let aggregations = get_reaction_aggregations(&state, &event_id).await?;

    Ok(Json(RelationsResponse {
        chunk: relations,
        aggregations,
        next_batch: None, // TODO: Implement pagination
        prev_batch: None,
    }))
}

async fn get_event_relations(
    state: &AppState,
    room_id: &str,
    event_id: &str,
    rel_type: Option<&str>,
    event_type: Option<&str>,
    limit: u32,
) -> Result<Vec<Event>, StatusCode> {
    let mut query = r#"
        SELECT e.*
        FROM events e
        WHERE e.room_id = $room_id 
        AND (e.content.m.relates_to.event_id = $event_id OR e.content['m.relates_to'].event_id = $event_id)
    "#.to_string();

    let mut params = HashMap::new();
    params.insert("room_id".to_string(), Value::String(room_id.to_string()));
    params.insert("event_id".to_string(), Value::String(event_id.to_string()));

    if let Some(rel_type) = rel_type {
        query.push_str(" AND (e.content.m.relates_to.rel_type = $rel_type OR e.content['m.relates_to'].rel_type = $rel_type)");
        params.insert("rel_type".to_string(), Value::String(rel_type.to_string()));
    }

    if let Some(event_type) = event_type {
        query.push_str(" AND e.type = $event_type");
        params.insert("event_type".to_string(), Value::String(event_type.to_string()));
    }

    query.push_str(" ORDER BY e.origin_server_ts DESC LIMIT $limit");
    params.insert("limit".to_string(), Value::Number(serde_json::Number::from(limit)));

    let result = state.database
        .query(&query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut events = Vec::new();

    if let Some(event_rows) = result.first() {
        for event_row in event_rows {
            let event = Event {
                content: event_row.get("content").cloned().unwrap_or(Value::Object(serde_json::Map::new())),
                event_id: event_row.get("event_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                origin_server_ts: event_row.get("origin_server_ts").and_then(|v| v.as_u64()).unwrap_or(0),
                room_id: event_row.get("room_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                sender: event_row.get("sender").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                event_type: event_row.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                unsigned: event_row.get("unsigned").cloned(),
            };

            events.push(event);
        }
    }

    Ok(events)
}

async fn get_reaction_aggregations(
    state: &AppState,
    event_id: &str,
) -> Result<HashMap<String, ReactionAggregation>, StatusCode> {
    let query = r#"
        SELECT reaction_key, count() as reaction_count, array::group(user_id) as users
        FROM event_reactions 
        WHERE event_id = $event_id
        GROUP BY reaction_key
        ORDER BY reaction_count DESC
    "#;

    let mut params = HashMap::new();
    params.insert("event_id".to_string(), Value::String(event_id.to_string()));

    let result = state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut aggregations = HashMap::new();

    if let Some(reaction_rows) = result.first() {
        for reaction_row in reaction_rows {
            if let (Some(reaction_key), Some(count), Some(users_array)) = (
                reaction_row.get("reaction_key").and_then(|v| v.as_str()),
                reaction_row.get("reaction_count").and_then(|v| v.as_u64()),
                reaction_row.get("users").and_then(|v| v.as_array()),
            ) {
                let users: Vec<String> = users_array
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();

                let aggregation = ReactionAggregation {
                    key: reaction_key.to_string(),
                    count,
                    users,
                };

                aggregations.insert(reaction_key.to_string(), aggregation);
            }
        }
    }

    Ok(aggregations)
}