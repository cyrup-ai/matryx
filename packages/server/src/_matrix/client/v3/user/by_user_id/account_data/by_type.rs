use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{info, error};


use crate::AppState;
use matryx_surrealdb::repository::ProfileManagementService;

#[derive(Serialize, Deserialize, Debug)]
pub struct AccountData {
    pub id: String,
    pub user_id: String,
    pub room_id: Option<String>,
    pub data_type: String,
    pub content: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct SetAccountDataRequest {
    #[serde(flatten)]
    pub content: Value,
}

#[derive(Serialize, Deserialize)]
pub struct DirectMessageData {
    #[serde(flatten)]
    pub user_rooms: HashMap<String, Vec<String>>,
}

#[derive(Serialize, Deserialize)]
pub struct IgnoredUserList {
    pub ignored_users: HashMap<String, Value>,
}

pub async fn get_account_data(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, data_type)): Path<(String, String)>,
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

    // Get account data using ProfileManagementService
    match profile_service.get_account_data(&user_id, &data_type, None).await {
        Ok(Some(content)) => {
            // Handle Matrix-standard account data types with proper structure validation
            match data_type.as_str() {
                "m.direct" => {
                    // Validate and return direct message data structure
                    match serde_json::from_value::<DirectMessageData>(content.clone()) {
                        Ok(_) => return Ok(Json(content)),
                        Err(_) => {
                            // If existing data is malformed, return empty direct message structure
                            let empty_dm_data = DirectMessageData {
                                user_rooms: HashMap::new(),
                            };
                            return Ok(Json(serde_json::to_value(empty_dm_data).unwrap_or(content)));
                        }
                    }
                },
                "m.ignored_user_list" => {
                    // Validate and return ignored user list structure
                    match serde_json::from_value::<IgnoredUserList>(content.clone()) {
                        Ok(_) => return Ok(Json(content)),
                        Err(_) => {
                            // If existing data is malformed, return empty ignored user list
                            let empty_ignored_list = IgnoredUserList {
                                ignored_users: HashMap::new(),
                            };
                            return Ok(Json(serde_json::to_value(empty_ignored_list).unwrap_or(content)));
                        }
                    }
                },
                _ => {
                    // For custom account data types, return as-is
                    return Ok(Json(content));
                }
            }
        },
        Ok(None) => {
            // Return appropriate default structure for Matrix-standard types
            match data_type.as_str() {
                "m.direct" => {
                    let default_dm_data = DirectMessageData {
                        user_rooms: HashMap::new(),
                    };
                    return Ok(Json(serde_json::to_value(default_dm_data).unwrap()));
                },
                "m.ignored_user_list" => {
                    let default_ignored_list = IgnoredUserList {
                        ignored_users: HashMap::new(),
                    };
                    return Ok(Json(serde_json::to_value(default_ignored_list).unwrap()));
                },
                _ => {
                    // For custom types, return 404 if not found
                    return Err(StatusCode::NOT_FOUND);
                }
            }
        },
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn set_account_data(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, data_type)): Path<(String, String)>,
    Json(request): Json<SetAccountDataRequest>,
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

    // Validate data type (Matrix spec: must not be empty)
    if data_type.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check for server-managed types that should be rejected (Matrix spec)
    if matches!(data_type.as_str(), "m.fully_read" | "m.push_rules") {
        return Err(StatusCode::METHOD_NOT_ALLOWED);
    }

    // Validate Matrix-standard account data types before storing
    let validated_content = match data_type.as_str() {
        "m.direct" => {
            // Validate direct message data structure
            match serde_json::from_value::<DirectMessageData>(request.content.clone()) {
                Ok(_) => request.content,
                Err(_) => {
                    error!("Invalid m.direct account data structure for user {}", user_id);
                    return Err(StatusCode::BAD_REQUEST);
                }
            }
        },
        "m.ignored_user_list" => {
            // Validate ignored user list structure
            match serde_json::from_value::<IgnoredUserList>(request.content.clone()) {
                Ok(_) => request.content,
                Err(_) => {
                    error!("Invalid m.ignored_user_list account data structure for user {}", user_id);
                    return Err(StatusCode::BAD_REQUEST);
                }
            }
        },
        _ => {
            // For custom account data types, accept as-is
            request.content
        }
    };

    let profile_service = ProfileManagementService::new(state.db.clone());

    // Set account data using ProfileManagementService
    match profile_service
        .set_account_data(&user_id, &data_type, validated_content, None)
        .await
    {
        Ok(()) => {},
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    info!("Account data updated for user {} type {}", user_id, data_type);

    Ok(Json(serde_json::json!({})))
}

// HTTP method handlers for main.rs routing
pub use get_account_data as get;
pub use set_account_data as put;
