use axum::{Json, extract::{Query, State}, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct LocationQuery {
    pub alias: Option<String>,
}

/// GET /_matrix/client/v3/thirdparty/location?alias={alias}
/// 
/// Reverse-lookup third-party locations given a Matrix room alias (query parameter).
/// Per Matrix spec, the alias is passed as a query parameter, not a path parameter.
pub async fn get(
    State(state): State<AppState>,
    Query(query): Query<LocationQuery>,
) -> Result<Json<Value>, StatusCode> {
    if let Some(alias) = query.alias {
        // Delegate to by_alias logic
        by_alias::get_by_alias(state, alias).await
    } else {
        // No alias provided, return empty array
        Ok(Json(json!([])))
    }
}

pub mod by_alias;
pub mod by_protocol;
