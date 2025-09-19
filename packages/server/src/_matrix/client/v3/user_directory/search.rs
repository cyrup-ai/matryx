use axum::{
    extract::State,
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
pub struct UserSearchRequest {
    pub search_term: String,
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct UserSearchResult {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
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
    let token_info = state.session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Validate search term
    if request.search_term.trim().is_empty() {
        return Ok(Json(UserSearchResponse {
            results: Vec::new(),
            limited: false,
        }));
    }

    if request.search_term.len() > 100 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Set defaults
    let limit = request.limit.unwrap_or(10).min(100);

    // Search for users with privacy filtering
    let results = search_user_directory(
        &state,
        &request.search_term,
        &token_info.user_id,
        limit,
    ).await?;

    let limited = results.len() >= limit as usize;

    Ok(Json(UserSearchResponse {
        results,
        limited,
    }))
}

async fn search_user_directory(
    state: &AppState,
    search_term: &str,
    searcher_user_id: &str,
    limit: u32,
) -> Result<Vec<UserSearchResult>, StatusCode> {
    // Privacy-aware search: only return users in shared rooms
    let search_query = r#"
        SELECT DISTINCT 
            u.user_id,
            up.display_name,
            up.avatar_url
        FROM users u
        LEFT JOIN user_profiles up ON u.user_id = up.user_id
        JOIN room_members rm1 ON u.user_id = rm1.user_id
        JOIN room_members rm2 ON rm1.room_id = rm2.room_id
        WHERE rm2.user_id = $searcher_user_id
        AND rm1.membership = 'join'
        AND rm2.membership = 'join'
        AND (
            u.user_id CONTAINS $search_term 
            OR up.display_name CONTAINS $search_term
            OR u.user_id ILIKE $search_pattern
            OR up.display_name ILIKE $search_pattern
        )
        AND u.user_id != $searcher_user_id
        ORDER BY 
            CASE 
                WHEN u.user_id = $search_term THEN 1
                WHEN up.display_name = $search_term THEN 2
                WHEN u.user_id STARTS WITH $search_term THEN 3
                WHEN up.display_name STARTS WITH $search_term THEN 4
                ELSE 5
            END,
            up.display_name,
            u.user_id
        LIMIT $limit
    "#;

    let search_pattern = format!("%{}%", search_term);

    let mut params = HashMap::new();
    params.insert("searcher_user_id".to_string(), Value::String(searcher_user_id.to_string()));
    params.insert("search_term".to_string(), Value::String(search_term.to_string()));
    params.insert("search_pattern".to_string(), Value::String(search_pattern));
    params.insert("limit".to_string(), Value::Number(serde_json::Number::from(limit)));

    let result = state.database
        .query(search_query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut users = Vec::new();

    if let Some(user_rows) = result.first() {
        for user_row in user_rows {
            if let Some(user_id) = user_row.get("user_id").and_then(|v| v.as_str()) {
                let user_result = UserSearchResult {
                    user_id: user_id.to_string(),
                    display_name: user_row.get("display_name").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    avatar_url: user_row.get("avatar_url").and_then(|v| v.as_str()).map(|s| s.to_string()),
                };

                users.push(user_result);
            }
        }
    }

    Ok(users)
}