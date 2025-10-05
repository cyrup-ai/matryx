use axum::extract::ConnectInfo;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};

use std::net::SocketAddr;
use tracing::{error, info};

use crate::state::AppState;
use matryx_surrealdb::repository::client_api_service::SearchCategories as RequestSearchCategories;
use matryx_surrealdb::repository::search::{
    EventContext, GroupBy, RoomEventFilter, SearchCategories as ResponseSearchCategories,
    SearchGroupings,
};
use matryx_surrealdb::repository::search::{SearchCriteria, SearchRepository};

#[derive(Deserialize)]
pub struct SearchRequest {
    pub search_categories: RequestSearchCategories,
}

#[derive(Serialize)]
pub struct SearchResponse {
    pub search_categories: ResponseSearchCategories,
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
    let token_info = state
        .session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user_id = token_info.user_id;

    info!("Search request from user {} at {}", user_id, addr);

    let mut response = SearchResponse {
        search_categories: ResponseSearchCategories { room_events: None },
    };

    // Handle room events search
    if let Some(room_events_criteria) = request.search_categories.room_events {
        info!("Searching room events for term: {}", room_events_criteria.search_term);

        // Create search repository
        let search_repo = SearchRepository::new(state.db.clone());

        // Build search criteria - convert from client API type to repository type
        let search_criteria = SearchCriteria {
            search_term: room_events_criteria.search_term.clone(),
            room_events: Some(matryx_surrealdb::repository::search::RoomEventsCriteria {
                search_term: room_events_criteria.search_term.clone(),
                keys: room_events_criteria.keys.clone(),
                filter: room_events_criteria.filter.as_ref().map(|f| RoomEventFilter {
                    limit: f.limit,
                    not_senders: f.not_senders.clone(),
                    not_types: f.not_types.clone(),
                    senders: f.senders.clone(),
                    types: f.types.clone(),
                    rooms: f.rooms.clone(),
                }),
                order_by: room_events_criteria.order_by.clone(),
                event_context: room_events_criteria.event_context.as_ref().map(|ec| EventContext {
                    before_limit: ec.before_limit,
                    after_limit: ec.after_limit,
                    include_profile: ec.include_profile,
                }),
                include_state: room_events_criteria.include_state,
                groupings: room_events_criteria.groupings.as_ref().map(|g| SearchGroupings {
                    group_by: g.group_by.as_ref().map(|gb| {
                        gb.iter().map(|item| GroupBy { key: item.key.clone() }).collect()
                    }),
                }),
            }),
            order_by: None,
            event_context: None,
            include_state: false,
            groupings: None,
        };

        // Perform search using repository
        match search_repo.search_events(&user_id, &search_criteria).await {
            Ok(search_results) => {
                response.search_categories = search_results.search_categories;
            },
            Err(e) => {
                error!("Search failed: {}", e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            },
        }
    }

    Ok(Json(response))
}
