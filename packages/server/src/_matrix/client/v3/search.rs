use axum::extract::ConnectInfo;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::net::SocketAddr;
use tracing::{error, info};

use crate::auth::MatrixAuthError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct SearchRequest {
    pub search_categories: SearchCategories,
}

#[derive(Deserialize)]
pub struct SearchCategories {
    pub room_events: Option<RoomEventsCriteria>,
}

#[derive(Deserialize)]
pub struct RoomEventsCriteria {
    pub search_term: String,
    pub keys: Option<Vec<String>>,
    pub filter: Option<RoomEventFilter>,
    pub order_by: Option<String>,
    pub event_context: Option<EventContext>,
    pub include_state: Option<bool>,
    pub groupings: Option<Groupings>,
}

#[derive(Deserialize)]
pub struct RoomEventFilter {
    pub limit: Option<u64>,
    pub not_senders: Option<Vec<String>>,
    pub not_types: Option<Vec<String>>,
    pub senders: Option<Vec<String>>,
    pub types: Option<Vec<String>>,
    pub rooms: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct EventContext {
    pub before_limit: Option<u64>,
    pub after_limit: Option<u64>,
    pub include_profile: Option<bool>,
}

#[derive(Deserialize)]
pub struct Groupings {
    pub group_by: Option<Vec<GroupBy>>,
}

#[derive(Deserialize)]
pub struct GroupBy {
    pub key: String,
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub search_categories: SearchResultCategories,
}

#[derive(Serialize)]
pub struct SearchResultCategories {
    pub room_events: Option<RoomEventsResults>,
}

#[derive(Serialize)]
pub struct RoomEventsResults {
    pub results: Vec<SearchResult>,
    pub count: Option<u64>,
    pub highlights: Vec<String>,
    pub next_batch: Option<String>,
    pub groups: Option<Value>,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub rank: f64,
    pub result: Value, // Event object
    pub context: Option<SearchResultContext>,
}

#[derive(Serialize)]
pub struct SearchResultContext {
    pub events_before: Vec<Value>,
    pub events_after: Vec<Value>,
    pub start: String,
    pub end: String,
    pub profile_info: Option<Value>,
}

/// POST /_matrix/client/v3/search
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, StatusCode> {
    // Extract access token from Authorization header
    let access_token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate access token using state's session service
    let token_info = state.session_service.validate_access_token(access_token).await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    let user_id = token_info.user_id;

    info!("Search request from user {} at {}", user_id, addr);

    let mut response = SearchResponse {
        search_categories: SearchResultCategories { room_events: None },
    };

    // Handle room events search
    if let Some(room_events_criteria) = request.search_categories.room_events {
        info!("Searching room events for term: {}", room_events_criteria.search_term);

        // Get user's accessible rooms
        let rooms_query = r#"
            SELECT room_id FROM room_members 
            WHERE user_id = $user_id AND membership = 'join'
        "#;

        let accessible_rooms: Vec<String> =
            match state.db.query(rooms_query).bind(("user_id", &user_id)).await {
                Ok(mut result) => {
                    match result.take::<Vec<(String,)>>(0) {
                        Ok(rooms) => rooms.into_iter().map(|(room_id,)| room_id).collect(),
                        Err(e) => {
                            error!("Failed to parse accessible rooms: {}", e);
                            return Err(StatusCode::INTERNAL_SERVER_ERROR);
                        },
                    }
                },
                Err(e) => {
                    error!("Failed to get accessible rooms: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                },
            };

        if accessible_rooms.is_empty() {
            // User has no accessible rooms, return empty results
            response.search_categories.room_events = Some(RoomEventsResults {
                results: vec![],
                count: Some(0),
                highlights: vec![],
                next_batch: None,
                groups: None,
            });
        } else {
            // Search in accessible rooms
            let search_query = r#"
                SELECT event_id, room_id, sender, event_type, content_body, origin_server_ts
                FROM search_index 
                WHERE room_id IN $rooms 
                AND content_body CONTAINS $search_term
                ORDER BY origin_server_ts DESC
                LIMIT 50
            "#;

            let search_results = match state
                .db
                .query(search_query)
                .bind(("rooms", &accessible_rooms))
                .bind(("search_term", &room_events_criteria.search_term))
                .await
            {
                Ok(mut result) => {
                    match result
                        .take::<Vec<(String, String, String, String, Option<String>, String)>>(0)
                    {
                        Ok(results) => results,
                        Err(e) => {
                            error!("Failed to parse search results: {}", e);
                            return Err(StatusCode::INTERNAL_SERVER_ERROR);
                        },
                    }
                },
                Err(e) => {
                    error!("Failed to execute search query: {}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                },
            };

            // Convert to search results format
            let mut results = Vec::new();
            for (event_id, room_id, sender, event_type, content_body, origin_server_ts) in
                search_results
            {
                // Get the full event from events table
                let event_query = "SELECT * FROM events WHERE event_id = $event_id";

                if let Ok(mut event_result) =
                    state.db.query(event_query).bind(("event_id", &event_id)).await
                {
                    if let Ok(events) = event_result.take::<Vec<Value>>(0) {
                        if let Some(event) = events.into_iter().next() {
                            results.push(SearchResult {
                                rank: 1.0, // Simple ranking for now
                                result: event,
                                context: None, // Context can be added later
                            });
                        }
                    }
                }
            }

            // Generate highlights from search term
            let highlights = vec![room_events_criteria.search_term.clone()];

            response.search_categories.room_events = Some(RoomEventsResults {
                count: Some(results.len() as u64),
                results,
                highlights,
                next_batch: None,
                groups: None,
            });
        }
    }

    Ok(Json(response))
}
