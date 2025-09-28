use axum::{Json, extract::{Path, Query, State}, http::{HeaderMap, StatusCode}};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info};

use crate::auth::{MatrixAuthError, extract_matrix_auth};
use crate::state::AppState;
use matryx_surrealdb::repository::third_party_service::ThirdPartyService;

#[derive(Deserialize)]
pub struct UserQuery {
    #[serde(flatten)]
    pub fields: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub userid: String,
    pub protocol: String,
    pub fields: HashMap<String, String>,
}

/// GET /_matrix/client/v3/thirdparty/user/{protocol}
pub async fn get(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(query): Query<UserQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserResponse>>, StatusCode> {
    // TASK17 SUBTASK 6: Authenticate user
    let auth_result = extract_matrix_auth(&headers, &state.session_service).await;
    let matrix_auth = match auth_result {
        Ok(auth) => auth,
        Err(MatrixAuthError::MissingToken) => return Err(StatusCode::UNAUTHORIZED),
        Err(MatrixAuthError::MissingAuthorization) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let user_id = match matrix_auth {
        crate::auth::MatrixAuth::User(user_auth) => user_auth.user_id,
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    info!("Third-party user lookup for protocol '{}' from user {}", protocol, user_id);

    // Use ThirdPartyService for user lookup
    let third_party_service = ThirdPartyService::new(state.db.clone());
    
    let users = match third_party_service.lookup_user(&protocol, &query.fields).await {
        Ok(users) => users,
        Err(e) => {
            error!("Failed to lookup third-party users for protocol '{}': {}", protocol, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Convert to response format
    let response: Vec<UserResponse> = users
        .into_iter()
        .map(|user| UserResponse {
            userid: user.userid,
            protocol: user.protocol,
            fields: user.fields,
        })
        .collect();

    Ok(Json(response))
}
