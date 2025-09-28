use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use matryx_surrealdb::repository::ProfileManagementService;
use serde::Deserialize;
use serde_json::Value;

use crate::AppState;

#[derive(Deserialize)]
pub struct SetRoomTagRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<f64>,
}

fn validate_tag_name(tag: &str) -> Result<(), &'static str> {
    // Matrix spec: tag names must not be empty and max 255 characters
    if tag.is_empty() {
        return Err("Tag name cannot be empty");
    }

    if tag.len() > 255 {
        return Err("Tag name too long (max 255 characters)");
    }

    // Matrix spec namespaces:
    // m.* - Matrix reserved tags (m.favourite, m.lowpriority, etc.)
    // u.* - User-defined tags
    // tld.name.* - Application-specific tags

    if tag.starts_with("m.") {
        // Validate known Matrix tags
        match tag {
            "m.favourite" | "m.lowpriority" | "m.server_notice" => Ok(()),
            _ if tag.starts_with("m.") => Err("Unknown Matrix reserved tag"),
            _ => Ok(()),
        }
    } else {
        // Allow user-defined and namespaced tags
        Ok(())
    }
}

fn validate_tag_order(order: Option<f64>) -> Result<(), &'static str> {
    if let Some(order_val) = order {
        // Matrix spec: order should be between 0 and 1 for proper sorting
        if !(0.0..=1.0).contains(&order_val) {
            return Err("Tag order must be between 0.0 and 1.0");
        }

        // Check for NaN or infinite values
        if !order_val.is_finite() {
            return Err("Tag order must be a finite number");
        }
    }
    Ok(())
}

pub async fn get_room_tag(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, room_id, tag)): Path<(String, String, String)>,
) -> Result<Json<Value>, StatusCode> {
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

    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Get room tag using ProfileManagementService
    match profile_service.get_room_tags(&user_id, &room_id).await {
        Ok(tags_response) => {
            if let Some(tag_content) = tags_response.tags.get(&tag) {
                Ok(Json(tag_content.clone()))
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        },
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn set_room_tag(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, room_id, tag)): Path<(String, String, String)>,
    Json(request): Json<SetRoomTagRequest>,
) -> Result<Json<Value>, StatusCode> {
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

    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate tag name
    validate_tag_name(&tag).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Validate tag order
    validate_tag_order(request.order).map_err(|_| StatusCode::BAD_REQUEST)?;

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Create tag content with order
    let tag_content = if let Some(order) = request.order {
        serde_json::json!({ "order": order })
    } else {
        serde_json::json!({})
    };

    // Set room tag using ProfileManagementService
    match profile_service
        .manage_room_tag(&user_id, &room_id, &tag, Some(tag_content))
        .await
    {
        Ok(()) => {},
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    Ok(Json(serde_json::json!({})))
}

pub async fn delete_room_tag(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, room_id, tag)): Path<(String, String, String)>,
) -> Result<Json<Value>, StatusCode> {
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

    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Remove room tag using ProfileManagementService
    match profile_service.remove_room_tag(&user_id, &room_id, &tag).await {
        Ok(()) => {},
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    Ok(Json(serde_json::json!({})))
}

// HTTP method handlers for main.rs routing
pub use delete_room_tag as delete;
pub use get_room_tag as get;
pub use set_room_tag as put;
