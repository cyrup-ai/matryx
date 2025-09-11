use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::state::AppState;
use matryx_entity::types::{Room, User};
use matryx_surrealdb::repository::{RoomRepository, UserRepository};

/// Query parameters for federation queries
#[derive(Debug, Deserialize)]
pub struct QueryParams {
    #[serde(flatten)]
    params: HashMap<String, String>,
}

/// Matrix X-Matrix authentication header parsed structure
#[derive(Debug, Clone)]
struct XMatrixAuth {
    origin: String,
    key_id: String,
    signature: String,
}

/// Parse X-Matrix authentication header
fn parse_x_matrix_auth(headers: &HeaderMap) -> Result<XMatrixAuth, StatusCode> {
    let auth_header = headers
        .get("authorization")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if !auth_header.starts_with("X-Matrix ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let auth_params = &auth_header[9..]; // Skip "X-Matrix "

    let mut origin = None;
    let mut key = None;
    let mut signature = None;

    for param in auth_params.split(',') {
        let param = param.trim();

        if let Some((key_name, value)) = param.split_once('=') {
            match key_name.trim() {
                "origin" => {
                    origin = Some(value.trim().to_string());
                },
                "key" => {
                    let key_value = value.trim().trim_matches('"');
                    if let Some(key_id) = key_value.strip_prefix("ed25519:") {
                        key = Some(key_id.to_string());
                    } else {
                        return Err(StatusCode::BAD_REQUEST);
                    }
                },
                "sig" => {
                    signature = Some(value.trim().trim_matches('"').to_string());
                },
                _ => {
                    // Unknown parameter, ignore for forward compatibility
                },
            }
        }
    }

    let origin = origin.ok_or(StatusCode::BAD_REQUEST)?;
    let key_id = key.ok_or(StatusCode::BAD_REQUEST)?;
    let signature = signature.ok_or(StatusCode::BAD_REQUEST)?;

    Ok(XMatrixAuth { origin, key_id, signature })
}

/// GET /_matrix/federation/v1/query/{queryType}
///
/// Generic endpoint for federation queries. Supports various query types
/// including directory, profile, and other information queries.
pub async fn get(
    State(state): State<AppState>,
    Path(query_type): Path<String>,
    Query(params): Query<QueryParams>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    // Parse X-Matrix authentication header
    let x_matrix_auth = parse_x_matrix_auth(&headers).map_err(|e| {
        warn!("Failed to parse X-Matrix authentication header: {}", e);
        e
    })?;

    debug!(
        "Federation query request - origin: {}, type: {}, params: {:?}",
        x_matrix_auth.origin, query_type, params.params
    );

    // Validate server signature
    let _server_validation = state
        .session_service
        .validate_server_signature(
            &x_matrix_auth.origin,
            &x_matrix_auth.key_id,
            &x_matrix_auth.signature,
            "GET",
            &format!("/_matrix/federation/v1/query/{}", query_type),
            &[],
        )
        .await
        .map_err(|e| {
            warn!("Server signature validation failed: {:?}", e);
            StatusCode::UNAUTHORIZED
        })?;

    // Route to appropriate query handler
    match query_type.as_str() {
        "directory" => handle_directory_query(&state, &params.params).await,
        "profile" => handle_profile_query(&state, &params.params).await,
        "client_versions" => handle_client_versions_query().await,
        _ => {
            warn!("Unknown query type: {}", query_type);
            Err(StatusCode::NOT_FOUND)
        },
    }
}

/// Handle directory queries (room alias resolution)
async fn handle_directory_query(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<Json<Value>, StatusCode> {
    let room_alias = params.get("room_alias").ok_or_else(|| {
        warn!("Missing room_alias parameter for directory query");
        StatusCode::BAD_REQUEST
    })?;

    debug!("Directory query for room alias: {}", room_alias);

    // Validate room alias format
    if !room_alias.starts_with('#') || !room_alias.contains(':') {
        warn!("Invalid room alias format: {}", room_alias);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Query database for room alias
    let query = "
        SELECT room_id, servers
        FROM room_alias
        WHERE alias = $alias
        LIMIT 1
    ";

    let mut response =
        state
            .db
            .query(query)
            .bind(("alias", room_alias.clone()))
            .await
            .map_err(|e| {
                error!("Failed to query room alias {}: {}", room_alias, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

    #[derive(serde::Deserialize)]
    struct AliasResult {
        room_id: String,
        servers: Option<Vec<String>>,
    }

    let alias_result: Option<AliasResult> = response.take(0).map_err(|e| {
        error!("Failed to parse alias result: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match alias_result {
        Some(alias) => {
            info!("Directory query successful for alias: {} -> {}", room_alias, alias.room_id);
            Ok(Json(json!({
                "room_id": alias.room_id,
                "servers": alias.servers.unwrap_or_else(|| vec![state.homeserver_name.clone()])
            })))
        },
        None => {
            warn!("Room alias not found: {}", room_alias);
            Err(StatusCode::NOT_FOUND)
        },
    }
}

/// Handle profile queries (user profile information)
async fn handle_profile_query(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<Json<Value>, StatusCode> {
    let user_id = params.get("user_id").ok_or_else(|| {
        warn!("Missing user_id parameter for profile query");
        StatusCode::BAD_REQUEST
    })?;

    debug!("Profile query for user: {}", user_id);

    // Validate user ID format
    if !user_id.starts_with('@') || !user_id.contains(':') {
        warn!("Invalid user ID format: {}", user_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check if user belongs to our server
    let server_part = user_id.split(':').nth(1).unwrap_or("");
    if server_part != state.homeserver_name {
        warn!("Profile query for non-local user: {}", user_id);
        return Err(StatusCode::NOT_FOUND);
    }

    // Query database for user profile
    let user_repo = Arc::new(UserRepository::new(state.db.clone()));
    let user = user_repo
        .get_by_id(user_id)
        .await
        .map_err(|e| {
            error!("Failed to query user {}: {}", user_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or_else(|| {
            warn!("User not found: {}", user_id);
            StatusCode::NOT_FOUND
        })?;

    info!("Profile query successful for user: {}", user_id);

    Ok(Json(json!({
        "user_id": user.user_id,
        "displayname": user.display_name,
        "avatar_url": user.avatar_url
    })))
}

/// Handle client versions queries
async fn handle_client_versions_query() -> Result<Json<Value>, StatusCode> {
    debug!("Client versions query");

    Ok(Json(json!({
        "versions": [
            "r0.0.1",
            "r0.1.0",
            "r0.2.0",
            "r0.3.0",
            "r0.4.0",
            "r0.5.0",
            "r0.6.0",
            "r0.6.1",
            "v1.1",
            "v1.2",
            "v1.3",
            "v1.4",
            "v1.5",
            "v1.6",
            "v1.7",
            "v1.8",
            "v1.9",
            "v1.10",
            "v1.11"
        ],
        "unstable_features": {}
    })))
}
