use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};


use crate::AppState;
use matryx_surrealdb::repository::user::{UserRepository, UserSearchResult};

#[derive(Deserialize)]
pub struct UserSearchRequest {
    pub search_term: String,
    pub limit: Option<u32>,
}



#[derive(Serialize)]
pub struct UserSearchResponse {
    pub results: Vec<UserSearchResult>,
    pub limited: bool,
}

/// POST /_matrix/client/v3/user_directory/search
pub async fn post(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UserSearchRequest>,
) -> Result<Json<UserSearchResponse>, StatusCode> {
    // Extract and validate access token
    let access_token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate token and get user context
    let token_info = state
        .session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Validate search term
    if request.search_term.trim().is_empty() {
        return Ok(Json(UserSearchResponse { results: Vec::new(), limited: false }));
    }

    if request.search_term.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Set defaults
    let limit = request.limit.unwrap_or(10).min(100);

    // Search for users with privacy filtering
    let results =
        search_user_directory(&state, &request.search_term, &token_info.user_id, limit).await?;

    let limited = results.len() >= limit as usize;

    Ok(Json(UserSearchResponse { results, limited }))
}

async fn search_user_directory(
    state: &AppState,
    search_term: &str,
    searcher_user_id: &str,
    limit: u32,
) -> Result<Vec<UserSearchResult>, StatusCode> {
    // Privacy-aware search: only return users in shared rooms
    let user_repo = UserRepository::new(state.db.clone());
    user_repo
        .search_users(searcher_user_id, search_term, limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
