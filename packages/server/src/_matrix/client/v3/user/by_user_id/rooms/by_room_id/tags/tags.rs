use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use matryx_surrealdb::repository::ProfileManagementService;

use crate::{
    auth::MatrixSessionService,
    AppState,
};

#[derive(Serialize, Deserialize)]
pub struct RoomTag {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<f64>,
}

#[derive(Serialize)]
pub struct RoomTagsResponse {
    pub tags: HashMap<String, RoomTag>,
}

pub async fn get_room_tags(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, room_id)): Path<(String, String)>,
) -> Result<Json<RoomTagsResponse>, StatusCode> {
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
    
    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let profile_service = ProfileManagementService::new(state.db.clone());
    
    // Get room tags using ProfileManagementService
    match profile_service.get_room_tags(&user_id, &room_id).await {
        Ok(tags_response) => {
            let mut tags = HashMap::new();
            
            for (tag_name, tag_content) in tags_response.tags {
                let order = tag_content.get("order").and_then(|v| v.as_f64());
                tags.insert(tag_name, RoomTag { order });
            }
            
            return Ok(Json(RoomTagsResponse { tags }));
        },
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}