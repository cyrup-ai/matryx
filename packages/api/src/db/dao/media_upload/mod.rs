use crate::db::client::DatabaseClient;
use crate::db::entity::media_upload::MediaUpload;
use crate::db::generic_dao::Dao;
use crate::future::MatrixFuture;
use chrono::Utc;
use serde_json::json;

/// MediaUpload DAO
#[derive(Clone)]
pub struct MediaUploadDao {
    dao: Dao<MediaUpload>,
}

impl MediaUploadDao {
    const TABLE_NAME: &'static str = "media_upload";

    /// Create a new MediaUploadDao
    pub fn new(client: DatabaseClient) -> Self {
        Self {
            dao: Dao::new(client, Self::TABLE_NAME),
        }
    }

    /// Mark a media upload as started
    pub fn mark_upload_started(&self, request_id: &str) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let request_id = request_id.to_string();

        MatrixFuture::spawn(async move {
            let now = Utc::now();

            // Try to update if exists
            let updated: Vec<MediaUpload> = dao.query_with_params::<Vec<MediaUpload>>(
                "UPDATE media_upload SET status = 'started', started_at = $now WHERE request_id = $id",
                json!({ "id": request_id, "now": now })
            ).await?;

            // If not updated, create new
            if updated.is_empty() {
                let mut upload = MediaUpload {
                    id: None,
                    request_id,
                    status: "started".to_string(),
                    started_at: now,
                    completed_at: None,
                };

                let _ = dao.create(&mut upload).await?;
            }

            Ok(())
        })
    }

    /// Get all active media uploads
    pub fn get_uploads(&self) -> MatrixFuture<Vec<String>> {
        let dao = self.dao.clone();

        MatrixFuture::spawn(async move {
            let uploads: Vec<MediaUpload> = dao
                .query_with_params::<Vec<MediaUpload>>(
                    "SELECT * FROM media_upload WHERE status = 'started'",
                    json!({}),
                )
                .await?;

            let request_ids = uploads.into_iter().map(|upload| upload.request_id).collect();

            Ok(request_ids)
        })
    }

    /// Mark a media upload as completed
    pub fn remove_upload(&self, request_id: &str) -> MatrixFuture<()> {
        let dao = self.dao.clone();
        let request_id = request_id.to_string();

        MatrixFuture::spawn(async move {
            let now = Utc::now();

            dao.query_with_params::<Vec<MediaUpload>>(
                "UPDATE media_upload SET status = 'completed', completed_at = $now WHERE request_id = $id",
                json!({ "id": request_id, "now": now })
            ).await?;

            Ok(())
        })
    }
}
