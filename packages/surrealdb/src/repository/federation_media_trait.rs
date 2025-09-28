use async_trait::async_trait;
use crate::repository::error::RepositoryError;
use crate::repository::media_service::MediaDownloadResult;

/// Trait for federation media client operations
/// 
/// This trait abstracts the interface for downloading media from remote Matrix servers,
/// enabling dependency injection and testing in the MediaService layer.
/// 
/// Implementations should handle:
/// - X-Matrix federation authentication
/// - Fallback from new federation endpoints to deprecated endpoints
/// - M_UNRECOGNIZED error detection
/// - Proper error mapping to RepositoryError
#[async_trait]
pub trait FederationMediaClientTrait: Send + Sync {
    /// Download media from a remote Matrix server with automatic fallback
    /// 
    /// This method should:
    /// 1. First attempt to download from the new federation endpoint: 
    ///    `/_matrix/federation/v1/media/download/{mediaId}`
    /// 2. If that returns 404 with M_UNRECOGNIZED error, fallback to:
    ///    `/_matrix/media/v3/download/{serverName}/{mediaId}?allow_remote=false`
    /// 3. Return appropriate RepositoryError variants for different failure modes
    /// 
    /// # Arguments
    /// * `server_name` - The Matrix server name hosting the media
    /// * `media_id` - The unique identifier for the media content
    /// 
    /// # Returns
    /// * `Ok(MediaDownloadResult)` - Successfully downloaded media with content and metadata
    /// * `Err(RepositoryError)` - Various error conditions:
    ///   - `NotFound` - Media not found on remote server
    ///   - `AccessDenied` - Access denied by remote server
    ///   - `InvalidOperation` - Network or protocol errors
    ///   - `InvalidData` - Malformed response data
    async fn download_media(
        &self,
        server_name: &str,
        media_id: &str,
    ) -> Result<MediaDownloadResult, RepositoryError>;
}