use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    auth::MatrixSessionService,
    database::SurrealRepository,
    AppState,
};

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
        if order_val < 0.0 || order_val > 1.0 {
            return Err("Tag order must be between 0.0 and 1.0");
        }
        
        // Check for NaN or infinite values
        if !order_val.is_finite() {
            return Err("Tag order must be a finite number");
        }
    }
    Ok(())
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
    let token_info = state.session_service
        .validate_access_token(access_token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    // Verify user authorization
    if token_info.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    // Validate tag name
    validate_tag_name(&tag)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // Validate tag order
    validate_tag_order(request.order)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

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

    // Generate unique ID for room tag
    let tag_id = Uuid::new_v4().to_string();

    // Update or create room tag
    let query = r#"
        UPDATE room_tags SET 
            tag_order = $tag_order,
            created_at = time::now()
        WHERE user_id = $user_id AND room_id = $room_id AND tag = $tag
        ELSE CREATE room_tags SET
            id = $id,
            user_id = $user_id,
            room_id = $room_id,
            tag = $tag,
            tag_order = $tag_order,
            created_at = time::now()
    "#;

    let mut params = HashMap::new();
    params.insert("id".to_string(), Value::String(tag_id));
    params.insert("user_id".to_string(), Value::String(user_id));
    params.insert("room_id".to_string(), Value::String(room_id));
    params.insert("tag".to_string(), Value::String(tag));
    params.insert("tag_order".to_string(), 
        request.order.map(|o| Value::Number(serde_json::Number::from_f64(o).unwrap()))
            .unwrap_or(Value::Null));

    state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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

    // Delete room tag
    let query = "DELETE FROM room_tags WHERE user_id = $user_id AND room_id = $room_id AND tag = $tag";
    let mut params = HashMap::new();
    params.insert("user_id".to_string(), Value::String(user_id));
    params.insert("room_id".to_string(), Value::String(room_id));
    params.insert("tag".to_string(), Value::String(tag));

    state.database
        .query(query, Some(params))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({})))
}