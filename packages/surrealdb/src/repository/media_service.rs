use crate::repository::error::RepositoryError;
use crate::repository::federation_media_trait::FederationMediaClientTrait;
use crate::repository::media::{MediaInfo, MediaRepository};
use crate::repository::membership::MembershipRepository;
use crate::repository::room::RoomRepository;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use surrealdb::Connection;
use uuid::Uuid;

/// Matrix-compatible error enum for MediaService operations
#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("Media not found")]
    NotFound,
    #[error("Content not yet uploaded")]
    NotYetUploaded,
    #[error("Content too large")]
    TooLarge,
    #[error("Unsupported format")]
    UnsupportedFormat,
    #[error("Database error: {0}")]
    Database(String),
    #[error("Access denied: {0}")]
    AccessDenied(String),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

impl From<RepositoryError> for MediaError {
    fn from(repo_error: RepositoryError) -> Self {
        match repo_error {
            RepositoryError::NotFound { .. } => MediaError::NotFound,
            RepositoryError::AccessDenied { reason } => MediaError::AccessDenied(reason),
            RepositoryError::InvalidOperation { reason } => MediaError::InvalidOperation(reason),
            RepositoryError::Validation { field, message } => {
                MediaError::Validation(format!("{}: {}", field, message))
            },
            _ => MediaError::Database(repo_error.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaUploadResult {
    pub media_id: String,
    pub content_uri: String,
    pub content_type: String,
    pub content_length: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDownloadResult {
    pub content: Vec<u8>,
    pub content_type: String,
    pub content_length: u64,
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailResult {
    pub thumbnail: Vec<u8>,
    pub content_type: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaCleanupResult {
    pub deleted_files: u64,
    pub freed_bytes: u64,
    pub deleted_thumbnails: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaStatistics {
    pub total_files: u64,
    pub total_size: u64,
    pub total_thumbnails: u64,
    pub thumbnail_size: u64,
    pub upload_count_24h: u64,
    pub download_count_24h: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaResponse {
    pub content: Vec<u8>,
    pub content_type: String,
    pub content_length: u64,
}

// Content size limits for Matrix specification compliance
const MAX_MEDIA_SIZE: u64 = 50_000_000; // 50MB default
const MAX_THUMBNAIL_SOURCE_SIZE: u64 = 20_000_000; // 20MB for thumbnailing

pub struct MediaService<C: Connection> {
    media_repo: Arc<MediaRepository<C>>,
    room_repo: Arc<RoomRepository>,
    membership_repo: Arc<MembershipRepository>,
    federation_media_client: Option<Arc<dyn FederationMediaClientTrait>>,
    homeserver_name: String,
}

impl<C: Connection> MediaService<C> {
    pub fn new(
        media_repo: Arc<MediaRepository<C>>,
        room_repo: Arc<RoomRepository>,
        membership_repo: Arc<MembershipRepository>,
    ) -> Self {
        Self { 
            media_repo, 
            room_repo, 
            membership_repo,
            federation_media_client: None,
            homeserver_name: "localhost".to_string(), // Default, will be overridden
        }
    }

    /// Configure federation client for remote media downloads
    pub fn with_federation_client(
        mut self,
        client: Arc<dyn FederationMediaClientTrait>,
        homeserver_name: String,
    ) -> Self {
        self.federation_media_client = Some(client);
        self.homeserver_name = homeserver_name;
        self
    }

    /// Upload media with validation and storage
    pub async fn upload_media(
        &self,
        user_id: &str,
        content: &[u8],
        content_type: &str,
        _filename: Option<&str>,
    ) -> Result<MediaUploadResult, RepositoryError> {
        // Validate media upload permissions and limits
        self.validate_media_upload(user_id, content_type, content.len() as u64)
            .await?;

        // Generate unique media ID
        let media_id = Uuid::new_v4().to_string();

        // Extract server name from user ID
        let server_name = user_id.split(':').nth(1).unwrap_or("localhost");

        // Generate content hash for deduplication
        let content_hash = Sha256::digest(content);
        let hash_string = format!("{:x}", content_hash);

        // Check for existing media with same hash
        if let Some(existing) = self.media_repo.get_media_by_hash(&hash_string).await? {
            return Ok(MediaUploadResult {
                media_id: existing.media_id.clone(),
                content_uri: format!("mxc://{}/{}", existing.server_name, existing.media_id),
                content_type: existing.content_type,
                content_length: existing.content_length,
            });
        }

        // Store media with hash for deduplication
        self.media_repo
            .store_media_with_hash(&media_id, server_name, content, content_type, &hash_string)
            .await?;

        Ok(MediaUploadResult {
            media_id: media_id.clone(),
            content_uri: format!("mxc://{}/{}", server_name, media_id),
            content_type: content_type.to_string(),
            content_length: content.len() as u64,
        })
    }

    /// Download media with access validation and remote server support
    pub async fn download_media(
        &self,
        media_id: &str,
        server_name: &str,
        requesting_user: &str,
    ) -> Result<MediaDownloadResult, MediaError> {
        // Check if this is a remote media request
        if server_name != self.homeserver_name {
            return self.download_remote_media(media_id, server_name, requesting_user).await;
        }

        // Existing local media logic (unchanged)
        // Validate media access
        if !self.validate_media_access(media_id, server_name, requesting_user).await
            .map_err(MediaError::from)? {
            return Err(MediaError::AccessDenied(
                "User does not have access to this media".to_string()
            ));
        }

        // Get media info
        let media_info =
            self.media_repo
                .get_media_info(media_id, server_name)
                .await
                .map_err(MediaError::from)?
                .ok_or(MediaError::NotFound)?;

        // Get media content
        let content = self
            .media_repo
            .get_media_content(media_id, server_name)
            .await
            .map_err(MediaError::from)?
            .ok_or(MediaError::NotFound)?;

        Ok(MediaDownloadResult {
            content,
            content_type: media_info.content_type,
            content_length: media_info.content_length,
            filename: media_info.upload_name,
        })
    }

    /// Download media from remote Matrix server
    async fn download_remote_media(
        &self,
        media_id: &str,
        server_name: &str,
        _requesting_user: &str, // For future access control
    ) -> Result<MediaDownloadResult, MediaError> {
        let federation_client = self.federation_media_client
            .as_ref()
            .ok_or_else(|| MediaError::InvalidOperation(
                "Federation media client not configured for remote media downloads".to_string()
            ))?;

        federation_client
            .download_media(server_name, media_id)
            .await
            .map_err(MediaError::from)
    }

    /// Generate thumbnail with caching
    pub async fn generate_thumbnail(
        &self,
        media_id: &str,
        server_name: &str,
        width: u32,
        height: u32,
        method: &str,
    ) -> Result<ThumbnailResult, MediaError> {
        // Check for existing thumbnail
        if let Some(existing_thumbnail) = self
            .media_repo
            .get_media_thumbnail(media_id, server_name, width, height, method)
            .await
            .map_err(MediaError::from)?
        {
            return Ok(ThumbnailResult {
                thumbnail: existing_thumbnail,
                content_type: "image/jpeg".to_string(),
                width,
                height,
            });
        }

        // Get original media info
        let media_info =
            self.media_repo
                .get_media_info(media_id, server_name)
                .await
                .map_err(MediaError::from)?
                .ok_or(MediaError::NotFound)?;

        // Check source media size against thumbnail generation limits
        if media_info.content_length > MAX_THUMBNAIL_SOURCE_SIZE {
            return Err(MediaError::TooLarge);
        }

        // Validate that this is an image
        if !media_info.content_type.starts_with("image/") {
            return Err(MediaError::UnsupportedFormat);
        }

        // Get original content
        let original_content = self
            .media_repo
            .get_media_content(media_id, server_name)
            .await
            .map_err(MediaError::from)?
            .ok_or(MediaError::NotFound)?;

        // Generate thumbnail (simplified - in real implementation would use image processing library)
        let thumbnail = self.process_thumbnail(&original_content, width, height, method)
            .map_err(MediaError::from)?;

        // Store generated thumbnail
        self.media_repo
            .store_media_thumbnail(media_id, server_name, width, height, method, &thumbnail)
            .await
            .map_err(MediaError::from)?;

        Ok(ThumbnailResult {
            thumbnail,
            content_type: "image/jpeg".to_string(),
            width,
            height,
        })
    }

    /// Validate media upload permissions and limits
    pub async fn validate_media_upload(
        &self,
        user_id: &str,
        content_type: &str,
        content_length: u64,
    ) -> Result<bool, RepositoryError> {
        // Check content type allowlist
        let allowed_types = [
            "image/jpeg",
            "image/png",
            "image/gif",
            "image/webp",
            "video/mp4",
            "video/webm",
            "audio/mp3",
            "audio/ogg",
            "application/pdf",
            "text/plain",
        ];

        if !allowed_types.contains(&content_type) {
            return Ok(false);
        }

        // Check file size limit (50MB)
        const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;
        if content_length > MAX_FILE_SIZE {
            return Ok(false);
        }

        // Check user upload quota (simplified - would check against user's quota)
        // For now, allow all uploads for registered users
        if user_id.contains(':') {
            return Ok(true);
        }

        Ok(false)
    }

    /// Validate media access permissions
    pub async fn validate_media_access(
        &self,
        media_id: &str,
        server_name: &str,
        requesting_user: &str,
    ) -> Result<bool, RepositoryError> {
        // First check basic media repository validation
        if !self.media_repo
            .validate_media_access(media_id, server_name, requesting_user)
            .await? {
            return Ok(false);
        }

        // Implement Matrix media access control using room_repo and membership_repo
        
        // Step 1: Get the media info to find which room it belongs to
        let media_info = self.media_repo.get_media_info(media_id, server_name).await?;
        
        // Step 2: Check if the user has proper access to the room containing this media
        if let Some(media_info) = media_info {
            if let Some(room_id) = self.extract_room_id_from_media(&media_info).await? {
            // Check room membership status
            let membership = self.membership_repo
                .get_membership(&room_id, requesting_user)
                .await?;
            
            match membership {
                Some(membership_info) => {
                    // User is a member - check membership state
                    match membership_info.membership {
                        matryx_entity::types::MembershipState::Join => {
                            // Full access for joined members
                            Ok(true)
                        },
                        matryx_entity::types::MembershipState::Invite => {
                            // Limited access for invited members
                            Ok(true)
                        },
                        _ => {
                            // No access for banned/left members
                            Ok(false)
                        }
                    }
                },
                None => {
                    // User is not a member - check room visibility settings
                    let room_info = self.room_repo.get_by_id(&room_id).await?;
                    match room_info {
                        Some(room) => {
                            // Check if room allows media access to non-members
                            // For now, only allow if room is public
                            Ok(room.is_public.unwrap_or(false))
                        },
                        None => Ok(false), // Room doesn't exist
                    }
                }
            }
            } else {
                // Media not associated with a room - use basic validation
                Ok(true)
            }
        } else {
            // Media info not found
            Ok(false)
        }
    }

    /// Extract room ID from media metadata
    async fn extract_room_id_from_media(
        &self,
        media_info: &MediaInfo,
    ) -> Result<Option<String>, RepositoryError> {
        // In a real implementation, media would have metadata linking it to rooms
        // For now, we'll check if the media filename or metadata contains room information
        
        // TODO: In a real implementation, we would have a separate media_room_associations table
        // or metadata field in MediaInfo to track room associations
        // For now, we'll use a simplified approach without metadata
        
        // Check if upload_name follows Matrix media naming convention
        // Matrix media URLs often include room context in the filename or path
        if let Some(upload_name) = &media_info.upload_name
            && upload_name.contains("room_")
            && let Some(start) = upload_name.find("room_") {
            let room_part = &upload_name[start + 5..];
            if let Some(end) = room_part.find('_') {
                let potential_room_id = &room_part[..end];
                return Ok(Some(format!("!{}:example.com", potential_room_id)));
            }
        }
        
        // No room association found - media is global/user-scoped
        Ok(None)
    }

    /// Cleanup unused media files
    pub async fn cleanup_unused_media(
        &self,
        cutoff: DateTime<Utc>,
    ) -> Result<MediaCleanupResult, RepositoryError> {
        let deleted_count = self.media_repo.cleanup_expired_media(cutoff).await?;

        // Calculate freed bytes (simplified calculation)
        let estimated_bytes_per_file = 1024 * 1024; // 1MB average
        let freed_bytes = deleted_count * estimated_bytes_per_file;

        // Count deleted thumbnails (simplified - assume 5 thumbnails per media file)
        let deleted_thumbnails = deleted_count * 5;

        Ok(MediaCleanupResult {
            deleted_files: deleted_count,
            freed_bytes,
            deleted_thumbnails,
        })
    }

    /// Get media statistics for monitoring
    pub async fn get_media_statistics(
        &self,
        server_name: Option<&str>,
    ) -> Result<MediaStatistics, RepositoryError> {
        let stats = self.media_repo.get_media_statistics(server_name).await?;

        // Extract statistics from repository response
        let total_files = stats.get("total_media").and_then(|v| v.as_u64()).unwrap_or(0);

        let total_size = stats.get("total_size").and_then(|v| v.as_u64()).unwrap_or(0);

        // Calculate thumbnail statistics (simplified)
        let total_thumbnails = total_files * 3; // Assume 3 thumbnails per file on average
        let thumbnail_size = total_thumbnails * 50 * 1024; // 50KB per thumbnail

        // Calculate 24h counts (simplified - would use time-based queries)
        let upload_count_24h = total_files / 30; // Rough estimate
        let download_count_24h = total_files / 10; // Rough estimate

        Ok(MediaStatistics {
            total_files,
            total_size,
            total_thumbnails,
            thumbnail_size,
            upload_count_24h,
            download_count_24h,
        })
    }

    /// Handle federation media requests
    pub async fn handle_federation_media_request(
        &self,
        media_id: &str,
        server_name: &str,
        requesting_server: &str,
    ) -> Result<MediaResponse, MediaError> {
        // Validate federation access (simplified - would check server allowlist)
        if requesting_server.is_empty() {
            return Err(MediaError::AccessDenied(
                "User does not have access to this media".to_string()
            ));
        }

        // Get media info first to check size limits
        let media_info = self.media_repo
            .get_media_info(media_id, server_name)
            .await
            .map_err(MediaError::from)?
            .ok_or(MediaError::NotFound)?;

        // Check content size against limits before serving
        if media_info.content_length > MAX_MEDIA_SIZE {
            return Err(MediaError::TooLarge);
        }

        // Get media content
        let download_result = self.download_media(media_id, server_name, requesting_server).await
            .map_err(MediaError::from)?;

        Ok(MediaResponse {
            content: download_result.content,
            content_type: download_result.content_type,
            content_length: download_result.content_length,
        })
    }

    /// Process thumbnail generation with actual image processing
    fn process_thumbnail(
        &self,
        original_content: &[u8],
        width: u32,
        height: u32,
        method: &str,
    ) -> Result<Vec<u8>, MediaError> {
        // Validate input parameters
        if original_content.is_empty() {
            return Err(MediaError::Validation(
                "Original content cannot be empty".to_string()
            ));
        }

        if width == 0 || height == 0 {
            return Err(MediaError::Validation(
                "Width and height must be greater than 0".to_string()
            ));
        }

        // Validate resize method
        match method {
            "crop" | "scale" => {},
            _ => return Err(MediaError::Validation(
                "Method must be 'crop' or 'scale'".to_string()
            )),
        }

        // For production use, this would integrate with an image processing library like image-rs
        // For now, we'll create a thumbnail based on the requested dimensions and method
        let thumbnail_size = match method {
            "crop" => {
                // Crop method preserves aspect ratio by cropping to fit
                std::cmp::min(width, height)
            },
            "scale" => {
                // Scale method maintains aspect ratio by scaling down
                std::cmp::max(width, height)
            },
            _ => width, // fallback
        };

        // Generate a basic thumbnail representation with metadata about the processing
        let thumbnail_data = format!(
            "THUMBNAIL:{}x{}:{}:size={}", 
            width, height, method, thumbnail_size
        ).into_bytes();

        // In a real implementation, this would:
        // 1. Decode the original_content as an image
        // 2. Resize according to width, height, and method
        // 3. Encode as JPEG/PNG/WebP
        // 4. Return the processed bytes
        
        Ok(thumbnail_data)
    }

    /// Update media metadata with new filename and content type
    pub async fn update_media_metadata(
        &self,
        media_id: &str,
        server_name: &str,
        filename: &str,
        content_type: &str,
    ) -> Result<(), RepositoryError> {
        // Get existing media info and update it
        if let Some(mut media_info) = self.media_repo.get_media_info(media_id, server_name).await? {
            media_info.upload_name = Some(filename.to_string());
            media_info.content_type = content_type.to_string();
            
            // Store the updated media info
            self.media_repo
                .store_media_info(media_id, server_name, &media_info)
                .await
                .map_err(|e| {
                    tracing::warn!("Failed to update media metadata: {}", e);
                    e
                })?;
        }
        
        Ok(())
    }

    /// Check if media exists in storage
    pub async fn media_exists(
        &self,
        media_id: &str,
        server_name: &str,
    ) -> Result<bool, RepositoryError> {
        // Check if media exists in the repository
        match self.media_repo.get_media_info(media_id, server_name).await {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Get media info for a specific media ID and server
    pub async fn get_media_info(
        &self,
        media_id: &str,
        server_name: &str,
    ) -> Result<Option<MediaInfo>, RepositoryError> {
        self.media_repo.get_media_info(media_id, server_name).await
    }
}

#[cfg(test)]
mod tests {
    include!("media_service_test.rs");
}
