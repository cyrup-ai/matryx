use axum::{Json, extract::{Query, State}, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct UserQuery {
    pub userid: Option<String>,
}

/// GET /_matrix/client/v3/thirdparty/user?userid={userid}
/// 
/// Reverse-lookup third-party users given a Matrix User ID (query parameter).
/// Per Matrix spec, the userid is passed as a query parameter, not a path parameter.
pub async fn get(
    State(state): State<AppState>,
    Query(query): Query<UserQuery>,
) -> Result<Json<Value>, StatusCode> {
    if let Some(userid) = query.userid {
        // Delegate to by_userid logic
        by_userid::get_by_userid(state, userid).await
    } else {
        // No userid provided, return empty array
        Ok(Json(json!([])))
    }
}

pub mod by_protocol;
pub mod by_userid;
