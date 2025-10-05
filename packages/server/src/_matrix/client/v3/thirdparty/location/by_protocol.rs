use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use tracing::{error, info};

use crate::auth::{MatrixAuthError, extract_matrix_auth};
use crate::state::AppState;
use matryx_surrealdb::repository::third_party_service::ThirdPartyService;

#[derive(Deserialize)]
pub struct LocationQuery {
    #[serde(flatten)]
    pub fields: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct LocationResponse {
    pub alias: String,
    pub protocol: String,
    pub fields: HashMap<String, String>,
}

/// GET /_matrix/client/v3/thirdparty/location/{protocol}
pub async fn get(
    State(state): State<AppState>,
    Path(protocol): Path<String>,
    Query(query): Query<LocationQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<LocationResponse>>, StatusCode> {
    // TASK17 SUBTASK 5: Authenticate user
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

    info!("Third-party location lookup for protocol '{}' from user {}", protocol, user_id);

    // Use ThirdPartyService for location lookup
    let third_party_service = ThirdPartyService::new(state.db.clone());

    let locations = match third_party_service.lookup_location(&protocol, &query.fields).await {
        Ok(locations) => locations,
        Err(e) => {
            error!("Failed to lookup third-party locations for protocol '{}': {}", protocol, e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        },
    };

    // Convert to response format
    let response: Vec<LocationResponse> = locations
        .into_iter()
        .map(|location| LocationResponse {
            alias: location.alias,
            protocol: location.protocol,
            fields: location.fields,
        })
        .collect();

    Ok(Json(response))
}
