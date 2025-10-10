use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use crate::{AppState, error::MatrixError};
use matryx_surrealdb::repository::media::MediaRepository;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct MarkIdpIconRequest {
    pub media_id: String,
    pub server_name: String,
    pub is_idp_icon: bool,
}

#[derive(Serialize)]
pub struct MarkIdpIconResponse {
    pub success: bool,
}

/// POST /_matrix/client/v1/admin/media/mark_idp_icon
/// Admin endpoint to mark/unmark media as IdP icon (exempt from freeze)
pub async fn mark_idp_icon(
    State(state): State<AppState>,
    Json(req): Json<MarkIdpIconRequest>,
) -> Result<Json<MarkIdpIconResponse>, MatrixError> {
    let media_repo = Arc::new(MediaRepository::new(state.db.clone()));

    let mut media_info = media_repo
        .get_media_info(&req.media_id, &req.server_name)
        .await
        .map_err(|_| MatrixError::Unknown)?
        .ok_or(MatrixError::NotFound)?;

    media_info.is_idp_icon = Some(req.is_idp_icon);

    media_repo
        .store_media_info(&req.media_id, &req.server_name, &media_info)
        .await
        .map_err(|_| MatrixError::Unknown)?;

    Ok(Json(MarkIdpIconResponse { success: true }))
}
