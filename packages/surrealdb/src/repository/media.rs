use crate::repository::error::RepositoryError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::{Connection, Surreal};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub media_id: String,
    pub server_name: String,
    pub content_type: String,
    pub content_length: u64,
    pub upload_name: Option<String>,
    pub uploaded_by: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    // Quarantine fields (from migration 158)
    #[serde(default)]
    pub quarantined: Option<bool>,
    #[serde(default)]
    pub quarantined_by: Option<String>,
    #[serde(default)]
    pub quarantine_reason: Option<String>,
    #[serde(default)]
    pub quarantined_at: Option<DateTime<Utc>>,
    /// Whether this media is an IdP icon for SSO (exempt from freeze)
    #[serde(default)]
    pub is_idp_icon: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUpload {
    pub media_id: String,
    pub server_name: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: PendingUploadStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PendingUploadStatus {
    Pending,
    Completed,
    Expired,
}

pub struct MediaRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> MediaRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Get media information
    pub async fn get_media_info(
        &self,
        media_id: &str,
        server_name: &str,
    ) -> Result<Option<MediaInfo>, RepositoryError> {
        let query = "
            SELECT * FROM media_info 
            WHERE media_id = $media_id AND server_name = $server_name
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .await?;
        let media_infos: Vec<MediaInfo> = result.take(0)?;
        Ok(media_infos.into_iter().next())
    }

    /// Store media information
    pub async fn store_media_info(
        &self,
        media_id: &str,
        server_name: &str,
        info: &MediaInfo,
    ) -> Result<(), RepositoryError> {
        let media_key = format!("{}:{}", server_name, media_id);
        let _: Option<MediaInfo> =
            self.db.create(("media_info", media_key)).content(info.clone()).await?;
        Ok(())
    }

    /// Get media content
    pub async fn get_media_content(
        &self,
        media_id: &str,
        server_name: &str,
    ) -> Result<Option<Vec<u8>>, RepositoryError> {
        let query = "
            SELECT content FROM media_content 
            WHERE media_id = $media_id AND server_name = $server_name
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .await?;
        let content_data: Vec<serde_json::Value> = result.take(0)?;

        if let Some(data) = content_data.first()
            && let Some(content_array) = data.get("content").and_then(|v| v.as_array())
        {
            let bytes: Vec<u8> = content_array
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            return Ok(Some(bytes));
        }

        Ok(None)
    }

    /// Store media content
    pub async fn store_media_content(
        &self,
        media_id: &str,
        server_name: &str,
        content: &[u8],
        content_type: &str,
    ) -> Result<(), RepositoryError> {
        let _media_key = format!("{}:{}", server_name, media_id);

        let query = "
            CREATE media_content SET
            media_id = $media_id,
            server_name = $server_name,
            content = $content,
            content_type = $content_type,
            content_length = $content_length,
            created_at = $created_at
        ";

        self.db
            .query(query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .bind(("content", content.to_vec()))
            .bind(("content_type", content_type.to_string()))
            .bind(("content_length", content.len() as i64))
            .bind(("created_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Get media thumbnail
    pub async fn get_media_thumbnail(
        &self,
        media_id: &str,
        server_name: &str,
        width: u32,
        height: u32,
        method: &str,
    ) -> Result<Option<Vec<u8>>, RepositoryError> {
        let query = "
            SELECT thumbnail_data FROM media_thumbnails 
            WHERE media_id = $media_id 
            AND server_name = $server_name
            AND width = $width 
            AND height = $height 
            AND method = $method
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .bind(("width", width as i64))
            .bind(("height", height as i64))
            .bind(("method", method.to_string()))
            .await?;
        let thumbnail_data: Vec<serde_json::Value> = result.take(0)?;

        if let Some(data) = thumbnail_data.first()
            && let Some(thumbnail_array) = data.get("thumbnail_data").and_then(|v| v.as_array())
        {
            let bytes: Vec<u8> = thumbnail_array
                .iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect();
            return Ok(Some(bytes));
        }

        Ok(None)
    }

    /// Store media thumbnail
    pub async fn store_media_thumbnail(
        &self,
        media_id: &str,
        server_name: &str,
        width: u32,
        height: u32,
        method: &str,
        thumbnail: &[u8],
    ) -> Result<(), RepositoryError> {
        let _thumbnail_key = format!("{}:{}:{}x{}:{}", server_name, media_id, width, height, method);

        let query = "
            CREATE media_thumbnails SET
            media_id = $media_id,
            server_name = $server_name,
            width = $width,
            height = $height,
            method = $method,
            thumbnail_data = $thumbnail_data,
            created_at = $created_at
        ";

        self.db
            .query(query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .bind(("width", width as i64))
            .bind(("height", height as i64))
            .bind(("method", method.to_string()))
            .bind(("thumbnail_data", thumbnail.to_vec()))
            .bind(("created_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Validate media access
    pub async fn validate_media_access(
        &self,
        media_id: &str,
        server_name: &str,
        _requesting_server: &str,
    ) -> Result<bool, RepositoryError> {
        if self.get_media_info(media_id, server_name).await?.is_none() {
            return Ok(false);
        }
        Ok(true)
    }

    /// Get media by content hash (for deduplication)
    pub async fn get_media_by_hash(
        &self,
        content_hash: &str,
    ) -> Result<Option<MediaInfo>, RepositoryError> {
        let query = "
            SELECT * FROM media_info 
            WHERE content_hash = $content_hash
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("content_hash", content_hash.to_string()))
            .await?;
        let media_infos: Vec<MediaInfo> = result.take(0)?;
        Ok(media_infos.into_iter().next())
    }

    /// Store media with content hash
    pub async fn store_media_with_hash(
        &self,
        media_id: &str,
        server_name: &str,
        content: &[u8],
        content_type: &str,
        content_hash: &str,
        user_id: &str,
    ) -> Result<(), RepositoryError> {
        // Store media info with hash
        let media_info = MediaInfo {
            media_id: media_id.to_string(),
            server_name: server_name.to_string(),
            content_type: content_type.to_string(),
            content_length: content.len() as u64,
            upload_name: None,
            uploaded_by: user_id.to_string(),
            created_at: Utc::now(),
            expires_at: None,
            quarantined: None,
            quarantined_by: None,
            quarantine_reason: None,
            quarantined_at: None,
            is_idp_icon: None,
        };

        self.store_media_info(media_id, server_name, &media_info).await?;

        // Store content
        self.store_media_content(media_id, server_name, content, content_type)
            .await?;

        // Store hash mapping
        let query = "
            CREATE media_hash_mapping SET
            content_hash = $content_hash,
            media_id = $media_id,
            server_name = $server_name,
            created_at = $created_at
        ";

        self.db
            .query(query)
            .bind(("content_hash", content_hash.to_string()))
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .bind(("created_at", Utc::now()))
            .await?;

        Ok(())
    }

    /// Delete media
    pub async fn delete_media(
        &self,
        media_id: &str,
        server_name: &str,
    ) -> Result<(), RepositoryError> {
        // Delete media info
        let info_key = format!("{}:{}", server_name, media_id);
        let _: Option<MediaInfo> = self.db.delete(("media_info", info_key)).await?;

        // Delete media content
        let query = "
            DELETE media_content 
            WHERE media_id = $media_id AND server_name = $server_name
        ";
        self.db
            .query(query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .await?;

        // Delete thumbnails
        let thumbnail_query = "
            DELETE media_thumbnails 
            WHERE media_id = $media_id AND server_name = $server_name
        ";
        self.db
            .query(thumbnail_query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .await?;

        Ok(())
    }

    /// Cleanup expired media
    pub async fn cleanup_expired_media(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<u64, RepositoryError> {
        let query = "
            SELECT media_id, server_name FROM media_info 
            WHERE expires_at IS NOT NULL AND expires_at < $cutoff
        ";
        let mut result = self.db.query(query).bind(("cutoff", cutoff)).await?;
        let expired_media: Vec<serde_json::Value> = result.take(0)?;

        let mut deleted_count = 0;
        for media in expired_media {
            if let (Some(media_id), Some(server_name)) = (
                media.get("media_id").and_then(|v| v.as_str()),
                media.get("server_name").and_then(|v| v.as_str()),
            ) && self.delete_media(media_id, server_name).await.is_ok() {
                deleted_count += 1;
            }
        }

        Ok(deleted_count)
    }

    /// Link media to a room for access control
    pub async fn associate_media_with_room(
        &self,
        media_id: &str,
        server_name: &str,
        room_id: &str,
        event_id: Option<&str>,
        uploaded_by: &str,
        is_profile_picture: bool,
    ) -> Result<(), RepositoryError> {
        let query = "
            CREATE media_room_associations SET
                media_id = $media_id,
                server_name = $server_name,
                room_id = $room_id,
                event_id = $event_id,
                uploaded_by = $uploaded_by,
                uploaded_at = time::now(),
                is_profile_picture = $is_profile_picture
        ";
        
        self.db
            .query(query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("event_id", event_id.map(|s| s.to_string())))
            .bind(("uploaded_by", uploaded_by.to_string()))
            .bind(("is_profile_picture", is_profile_picture))
            .await?;
        
        Ok(())
    }

    /// Quarantine media (admin only)
    pub async fn quarantine_media(
        &self,
        media_id: &str,
        server_name: &str,
        admin_user: &str,
        reason: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE media_info SET
                quarantined = true,
                quarantined_by = $admin_user,
                quarantine_reason = $reason,
                quarantined_at = time::now()
            WHERE media_id = $media_id AND server_name = $server_name
        ";
        
        self.db
            .query(query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .bind(("admin_user", admin_user.to_string()))
            .bind(("reason", reason.to_string()))
            .await?;
        
        Ok(())
    }

    /// Get user's total media storage usage
    pub async fn get_user_storage_usage(
        &self,
        user_id: &str,
        server_name: &str,
    ) -> Result<u64, RepositoryError> {
        let query = "
            SELECT VALUE sum(content_length) FROM media_info
            WHERE uploaded_by = $user_id AND server_name = $server_name
            GROUP ALL
        ";

        let mut result = self.db.query(query)
            .bind(("user_id", user_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .await?;

        let user_bytes: Vec<Option<u64>> = result.take(0)?;
        Ok(user_bytes.first().and_then(|v| *v).unwrap_or(0))
    }

    /// Get room association for media
    pub async fn get_media_room_association(
        &self,
        media_id: &str,
        server_name: &str,
    ) -> Result<Option<(String, bool)>, RepositoryError> {
        let query = "
            SELECT room_id, is_profile_picture 
            FROM media_room_associations 
            WHERE media_id = $media_id AND server_name = $server_name
            LIMIT 1
        ";
        
        let mut result = self.db.query(query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .await?;
        
        let associations: Vec<serde_json::Value> = result.take(0)?;
        
        if let Some(assoc) = associations.first() {
            let is_profile = assoc.get("is_profile_picture")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            
            if let Some(room_id) = assoc.get("room_id").and_then(|v| v.as_str()) {
                return Ok(Some((room_id.to_string(), is_profile)));
            }
        }
        
        Ok(None)
    }

    /// Get media statistics
    pub async fn get_media_statistics(
        &self,
        server_name: Option<&str>,
    ) -> Result<serde_json::Value, RepositoryError> {
        let query = if let Some(_server) = server_name {
            "
            SELECT 
                count() as total_media,
                sum(content_length) as total_size
            FROM media_info 
            WHERE server_name = $server_name
            GROUP ALL
            "
        } else {
            "
            SELECT 
                count() as total_media,
                sum(content_length) as total_size
            FROM media_info
            GROUP ALL
            "
        };

        let mut result = if let Some(server) = server_name {
            self.db.query(query).bind(("server_name", server.to_string())).await?
        } else {
            self.db.query(query).await?
        };

        let stats: Vec<serde_json::Value> = result.take(0)?;
        Ok(stats.into_iter().next().unwrap_or(serde_json::json!({})))
    }

    /// Create a pending upload reservation
    pub async fn create_pending_upload(
        &self,
        media_id: &str,
        server_name: &str,
        created_by: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let pending_upload = PendingUpload {
            media_id: media_id.to_string(),
            server_name: server_name.to_string(),
            created_by: created_by.to_string(),
            created_at: Utc::now(),
            expires_at,
            status: PendingUploadStatus::Pending,
        };

        let pending_key = format!("{}:{}", server_name, media_id);
        let _: Option<PendingUpload> = self
            .db
            .create(("pending_uploads", pending_key))
            .content(pending_upload)
            .await?;

        Ok(())
    }

    /// Get a pending upload by media_id and server_name
    pub async fn get_pending_upload(
        &self,
        media_id: &str,
        server_name: &str,
    ) -> Result<Option<PendingUpload>, RepositoryError> {
        let query = "
            SELECT * FROM pending_uploads
            WHERE media_id = $media_id AND server_name = $server_name
            LIMIT 1
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .await?;
        let pending_uploads: Vec<PendingUpload> = result.take(0)?;
        Ok(pending_uploads.into_iter().next())
    }

    /// Count pending uploads for a user (for rate limiting)
    pub async fn count_user_pending_uploads(
        &self,
        user_id: &str,
    ) -> Result<u64, RepositoryError> {
        let query = "
            SELECT VALUE count() FROM pending_uploads
            WHERE created_by = $user_id AND status = 'Pending'
            GROUP ALL
        ";
        let mut result = self
            .db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;
        let counts: Vec<Option<u64>> = result.take(0)?;
        Ok(counts.into_iter().next().and_then(|v| v).unwrap_or(0))
    }

    /// Mark a pending upload as completed
    pub async fn mark_pending_upload_completed(
        &self,
        media_id: &str,
        server_name: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            UPDATE pending_uploads SET
                status = 'Completed'
            WHERE media_id = $media_id AND server_name = $server_name
        ";
        self.db
            .query(query)
            .bind(("media_id", media_id.to_string()))
            .bind(("server_name", server_name.to_string()))
            .await?;
        Ok(())
    }

    /// Cleanup expired pending uploads
    pub async fn cleanup_expired_pending_uploads(&self) -> Result<u64, RepositoryError> {
        let now = Utc::now();
        let query = "
            SELECT media_id, server_name FROM pending_uploads
            WHERE expires_at < $now AND status = 'Pending'
        ";
        let mut result = self.db.query(query).bind(("now", now)).await?;
        let expired_uploads: Vec<serde_json::Value> = result.take(0)?;

        let mut deleted_count = 0;
        for upload in expired_uploads {
            if let (Some(media_id), Some(server_name)) = (
                upload.get("media_id").and_then(|v| v.as_str()),
                upload.get("server_name").and_then(|v| v.as_str()),
            ) {
                let delete_query = "
                    DELETE pending_uploads
                    WHERE media_id = $media_id AND server_name = $server_name
                ";
                if self
                    .db
                    .query(delete_query)
                    .bind(("media_id", media_id.to_string()))
                    .bind(("server_name", server_name.to_string()))
                    .await
                    .is_ok()
                {
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
    }
}
