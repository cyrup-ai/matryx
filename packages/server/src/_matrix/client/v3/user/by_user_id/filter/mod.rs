use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use matryx_entity::types::MatrixFilter;
use matryx_surrealdb::repository::filter::FilterRepository;
use serde_json::{Value, json};
use uuid::Uuid;

/// POST /_matrix/client/v3/user/{userId}/filter
pub async fn post(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    auth: AuthenticatedUser,
    Json(filter): Json<MatrixFilter>,
) -> Result<Json<Value>, StatusCode> {
    if auth.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let filter_id = Uuid::new_v4().to_string();

    let filter_repo = FilterRepository::new(state.db.clone());
    filter_repo
        .create(&filter, &filter_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "filter_id": filter_id
    })))
}

pub mod by_filter_id;
