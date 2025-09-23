use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use matryx_entity::types::MatrixFilter;
use matryx_surrealdb::repository::filter::FilterRepository;

/// GET /_matrix/client/v3/user/{userId}/filter/{filterId}
pub async fn get(
    State(state): State<AppState>,
    Path((user_id, filter_id)): Path<(String, String)>,
    auth: AuthenticatedUser,
) -> Result<Json<MatrixFilter>, StatusCode> {
    if auth.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let filter_repo = FilterRepository::new(state.db.clone());
    let filter = filter_repo
        .get_by_id(&filter_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(filter))
}
