use crate::repository::error::RepositoryError;
use base64::{Engine as _, engine::general_purpose};
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
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
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

        if let Some(data) = content_data.first() {
            if let Some(content) = data.get("content").and_then(|v| v.as_str()) {
                // In a real implementation, this would be stored as binary data
                // For now, we'll decode from base64
                if let Ok(bytes) = general_purpose::STANDARD.decode(content) {
                    return Ok(Some(bytes));
                }
            }
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

        // Encode content as base64 for storage
        let encoded_content = general_purpose::STANDARD.encode(content);

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
            .bind(("content", encoded_content))
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

        if let Some(data) = thumbnail_data.first() {
            if let Some(thumbnail) = data.get("thumbnail_data").and_then(|v| v.as_str()) {
                if let Ok(bytes) = general_purpose::STANDARD.decode(thumbnail) {
                    return Ok(Some(bytes));
                }
            }
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

        // Encode thumbnail as base64 for storage
        let encoded_thumbnail = general_purpose::STANDARD.encode(thumbnail);

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
            .bind(("thumbnail_data", encoded_thumbnail))
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
        // Check if media exists
        if self.get_media_info(media_id, server_name).await?.is_none() {
            return Ok(false);
        }

        // Check if requesting server has access
        // In a real implementation, this would check:
        // 1. If the media is public
        // 2. If the requesting server is in the same room as the media
        // 3. If there are specific access controls

        // For now, allow access if the media exists
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
    ) -> Result<(), RepositoryError> {
        // Store media info with hash
        let media_info = MediaInfo {
            media_id: media_id.to_string(),
            server_name: server_name.to_string(),
            content_type: content_type.to_string(),
            content_length: content.len() as u64,
            upload_name: None,
            created_at: Utc::now(),
            expires_at: None,
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
            ) {
                if self.delete_media(media_id, server_name).await.is_ok() {
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
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
}
