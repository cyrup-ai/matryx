use axum::extract::ConnectInfo;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::net::SocketAddr;
use tracing::{error, info};

use crate::auth::{MatrixAuthError, MatrixSessionService};
use crate::state::AppState;

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
    let token_info = state.session_service.validate_access_token(access_token).await
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

    if is_deletion {
        // Delete existing pusher
        let delete_query =
            "DELETE FROM pushers WHERE user_id = $user_id AND pusher_id = $pusher_id";

        if let Err(e) = state
            .db
            .query(delete_query)
            .bind(("user_id", &user_id))
            .bind(("pusher_id", &request.pusher_id))
            .await
        {
            error!("Failed to delete pusher: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }

        info!("Deleted pusher {} for user {}", request.pusher_id, user_id);
    } else {
        // Create or update pusher
        let pusher_id = format!("pusher_{}", uuid::Uuid::new_v4());

        let upsert_query = r#"
            UPSERT pushers SET
                id = $id,
                user_id = $user_id,
                pusher_id = $pusher_id,
                kind = $kind,
                app_id = $app_id,
                app_display_name = $app_display_name,
                device_display_name = $device_display_name,
                profile_tag = $profile_tag,
                lang = $lang,
                data = $data,
                created_at = time::now()
            WHERE user_id = $user_id AND pusher_id = $pusher_id
        "#;

        let data_json = json!({
            "url": request.data.url,
            "format": request.data.format.unwrap_or_else(|| "event_id_only".to_string())
        });

        if let Err(e) = state
            .db
            .query(upsert_query)
            .bind(("id", &pusher_id))
            .bind(("user_id", &user_id))
            .bind(("pusher_id", &request.pusher_id))
            .bind(("kind", &request.kind))
            .bind(("app_id", &request.app_id))
            .bind(("app_display_name", &request.app_display_name))
            .bind(("device_display_name", &request.device_display_name))
            .bind(("profile_tag", &request.profile_tag))
            .bind(("lang", &request.lang))
            .bind(("data", &data_json))
            .await
        {
            error!("Failed to set pusher: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }

        info!(
            "Set pusher {} for user {} with URL {:?}",
            request.pusher_id, user_id, request.data.url
        );
    }

    Ok(Json(json!({})))
}
