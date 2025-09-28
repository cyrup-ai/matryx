use axum::extract::ConnectInfo;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::net::SocketAddr;
use tracing::{error, info};

use crate::state::AppState;
use matryx_surrealdb::repository::push::{PushRepository, PusherConfig};

#[derive(Deserialize)]
pub struct SetPusherRequest {
    pub pusher_id: String,
    pub kind: String,
    pub app_id: String,
    pub app_display_name: String,
    pub device_display_name: String,
    pub profile_tag: Option<String>,
    pub lang: String,
    pub data: PusherData,
    pub append: Option<bool>,
}

#[derive(Deserialize)]
pub struct PusherData {
    pub url: Option<String>,
    pub format: Option<String>,
}

/// POST /_matrix/client/v3/pushers/set
pub async fn post(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<SetPusherRequest>,
) -> Result<Json<Value>, StatusCode> {
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

    info!(
        "Setting pusher for user {} from {}: pusher_id={}, kind={}, app_id={}",
        user_id, addr, request.pusher_id, request.kind, request.app_id
    );

    // Validate pusher kind
    if request.kind != "http" && request.kind != "email" {
        error!("Invalid pusher kind: {}", request.kind);
        return Err(StatusCode::BAD_REQUEST);
    }

    // For HTTP pushers, validate URL
    if request.kind == "http" {
        if let Some(ref url) = request.data.url {
            if !url.starts_with("https://") && !url.starts_with("http://") {
                error!("Invalid pusher URL: {}", url);
                return Err(StatusCode::BAD_REQUEST);
            }
        } else {
            error!("HTTP pusher missing URL");
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Check if this is a pusher deletion (null data)
    let is_deletion = request.data.url.is_none() && request.kind == "http";

    let push_repo = PushRepository::new(state.db.clone());

    if is_deletion {
        // Delete existing pusher
        if let Err(e) = push_repo.delete_pusher(&user_id, &request.pusher_id).await {
            error!("Failed to delete pusher: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }

        info!("Deleted pusher {} for user {}", request.pusher_id, user_id);
    } else {
        // Handle append logic according to Matrix spec
        // append=false (default): replace all pushers for this app_id with this pusher
        // append=true: add this pusher alongside existing ones
        let append_mode = request.append.unwrap_or(false);
        
        if !append_mode {
            // Replace mode: delete existing pushers for this app_id first
            if let Err(e) = push_repo.delete_pushers_by_app_id(&user_id, &request.app_id).await {
                error!("Failed to delete existing pushers for app_id {}: {}", request.app_id, e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
            info!("Cleared existing pushers for app_id {} (replace mode)", request.app_id);
        }

        // Create or update pusher
        let data_json = json!({
            "url": request.data.url,
            "format": request.data.format.unwrap_or_else(|| "event_id_only".to_string())
        });

        let config = PusherConfig {
            kind: &request.kind,
            app_id: &request.app_id,
            app_display_name: &request.app_display_name,
            device_display_name: &request.device_display_name,
            profile_tag: request.profile_tag.as_deref(),
            lang: &request.lang,
            data: &data_json,
        };

        if let Err(e) = push_repo
            .upsert_pusher(
                &user_id,
                &request.pusher_id,
                config,
            )
            .await
        {
            error!("Failed to set pusher: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }

        info!(
            "Set pusher {} for user {} with URL {:?} (append={})",
            request.pusher_id, user_id, request.data.url, append_mode
        );
    }

    Ok(Json(json!({})))
}
