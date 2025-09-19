use axum::{
    extract::{Path, State},
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

    // Verify user is member of the room
    let membership_query = "SELECT membership FROM room_members WHERE room_id = $room_id AND user_id = $user_id";
    let mut membership_params = HashMap::new();
    membership_params.insert("room_id".to_string(), Value::String(room_id.clone()));
    membership_params.insert("user_id".to_string(), Value::String(user_id.clone()));

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

    // Query room tags from database
    let query = "SELECT tag, tag_order FROM room_tags WHERE user_id = $user_id AND room_id = $room_id";
    let mut params = HashMap::new();
    params.insert("user_id".to_string(), Value::String(user_id));
    params.insert("room_id".to_string(), Value::String(room_id));

    let result = state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut tags = HashMap::new();

    if let Some(tag_rows) = result.first() {
        for tag_row in tag_rows {
            if let (Some(tag_name), tag_order) = (
                tag_row.get("tag").and_then(|v| v.as_str()),
                tag_row.get("tag_order").and_then(|v| v.as_f64()),
            ) {
                tags.insert(tag_name.to_string(), RoomTag {
                    order: tag_order,
                });
            }
        }
    }

    Ok(Json(RoomTagsResponse { tags }))
}