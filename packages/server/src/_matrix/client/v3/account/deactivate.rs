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
pub struct DeactivateAccountRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Value>, // Authentication data for verification
    #[serde(default)]
    pub erase: bool, // Whether to erase all user data
}

#[derive(Serialize)]
pub struct DeactivateAccountResponse {
    pub id_server_unbind_result: String,
}

pub async fn deactivate_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeactivateAccountRequest>,
) -> Result<Json<DeactivateAccountResponse>, StatusCode> {
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

    // TODO: Validate auth data if provided (password confirmation, etc.)
    // For now, we'll proceed without additional auth validation

    let user_id = &token_info.user_id;

    // Begin account deactivation process
    if request.erase {
        // Erase all user data
        let erase_queries = vec![
            "DELETE FROM user_profiles WHERE user_id = $user_id",
            "DELETE FROM account_data WHERE user_id = $user_id", 
            "DELETE FROM room_tags WHERE user_id = $user_id",
            "DELETE FROM user_threepids WHERE user_id = $user_id",
            "DELETE FROM media_files WHERE uploaded_by = $user_id",
            "DELETE FROM devices WHERE user_id = $user_id",
            "DELETE FROM access_tokens WHERE user_id = $user_id",
            "DELETE FROM room_members WHERE user_id = $user_id",
            "DELETE FROM users WHERE user_id = $user_id",
        ];

        let mut params = HashMap::new();
        params.insert("user_id".to_string(), Value::String(user_id.clone()));

        for query in erase_queries {
            state.database
                .query(query, Some(params.clone()))
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    } else {
        // Just deactivate the account (mark as inactive)
        let query = "UPDATE users SET active = false, deactivated_at = time::now() WHERE user_id = $user_id";
        let mut params = HashMap::new();
        params.insert("user_id".to_string(), Value::String(user_id.clone()));

        state.database
            .query(query, Some(params))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Invalidate all access tokens for this user
        let token_query = "DELETE FROM access_tokens WHERE user_id = $user_id";
        let mut token_params = HashMap::new();
        token_params.insert("user_id".to_string(), Value::String(user_id.clone()));

        state.database
            .query(token_query, Some(token_params))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(Json(DeactivateAccountResponse {
        id_server_unbind_result: "success".to_string(),
    }))
}