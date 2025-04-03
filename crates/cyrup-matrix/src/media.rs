//! Media manager wrapper with synchronous interfaces
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Media functionality
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Handle;

use matrix_sdk::{
    media::{MediaFormat, MediaRequest, MediaSource, MediaThumbnailSettings}, // Import MediaFormat, MediaRequest, MediaSource
    ruma::{
        api::client::media::get_content_thumbnail::v3::Method as ThumbnailMethod, MxcUri, // Import MxcUri
        OwnedMxcUri, // Import OwnedMxcUri
        UInt,
    },
    Client as MatrixClient,
};
use tracing::warn; // Add warn import

use crate::error::MediaError;
use crate::future::MatrixFuture;
    Client as MatrixClient,
};

use crate::error::MediaError;
use crate::future::MatrixFuture;

/// A synchronous wrapper around the Matrix SDK Media functionality.
///
/// This wrapper enables using the Media manager with a synchronous interface,
/// hiding all async complexity behind MatrixFuture objects that properly
/// implement the Future trait.
pub struct CyrumMedia {
    client: Arc<MatrixClient>,
    runtime_handle: Handle,
}

impl CyrumMedia {
    /// Create a new CyrumMedia with the provided Matrix client.
    pub fn new(client: Arc<MatrixClient>) -> Self {
        Self { client, runtime_handle: Handle::current() }
    }

    /// Upload content to the homeserver.
    pub fn upload(
        &self,
        content_type: &str,
        data: Vec<u8>,
        filename: Option<&str>,
    ) -> MatrixFuture<OwnedMxcUri> { // Return OwnedMxcUri
        let client = self.client.clone();
        let filename = filename.map(|s| s.to_owned()); // Clone filename for async block

        MatrixFuture::spawn(async move {
            let content_type = content_type.parse().map_err(|_| MediaError::InvalidParameter("Invalid content type".into()))?; // Parse content type inside async
            let data = data.into(); // Convert Vec<u8> to Bytes
            // Use the media uploader
            let request = client.media().upload(&content_type, data);
            let request = if let Some(name) = filename.as_deref() { // Use cloned filename
                request.file_name(name) // Use builder pattern for filename
            } else {
                request
            };

            let result = request.await;

            // Map MediaError to crate::error::Error
            result.map(|response| response.content_uri).map_err(crate::error::Error::Media)
        })
    }

    /// Upload a file from disk to the homeserver.
    pub fn upload_file(&self, path: PathBuf) -> MatrixFuture<OwnedMxcUri> { // Return OwnedMxcUri
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let result = async {
                // Read file content
                let data = match tokio::fs::read(&path).await {
                Ok(data) => data,
                Err(e) => return Err(MediaError::IoError(e.to_string())),
            };

            // Get file name and extension for content type
            let filename = path.file_name().and_then(|name| name.to_str()).unwrap_or("file");
            let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

            // Determine content type based on extension (simple mapping)
            let content_type = match extension {
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "svg" => "image/svg+xml",
                "mp4" => "video/mp4",
                "mp3" => "audio/mpeg",
                "ogg" => "audio/ogg",
                "wav" => "audio/wav",
                "pdf" => "application/pdf",
                "txt" => "text/plain",
                "html" => "text/html",
                "css" => "text/css",
                "js" => "application/javascript",
                "json" => "application/json",
                "xml" => "application/xml",
                _ => "application/octet-stream",
            }
            .parse::<mime::Mime>() // Parse the determined mime type string
            .map_err(|_| MediaError::InvalidParameter("Could not parse mime type".into()))?;

            // Use the media uploader
            let response = client.media()
                    .upload(&content_type, data.into()) // Pass parsed mime and converted data
                    .file_name(filename) // Set filename
                    .await
                    .map_err(MediaError::matrix_sdk)?;

                Ok(response.content_uri)
            }.await;

            // Map MediaError to crate::error::Error
            result.map_err(crate::error::Error::Media)
        })
    }

    /// Download content from the homeserver.
    pub fn download(&self, mxc_uri: &str) -> MatrixFuture<Vec<u8>> {
        let mxc_uri_owned = mxc_uri.to_owned(); // Clone for the async block
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let result = async {
                // Parse the MXC URI
                let uri = MxcUri::parse(&mxc_uri_owned)
                     .map_err(|e| MediaError::InvalidUri(e.to_string()))?;

                // Create a media request
                let request = MediaRequest {
                    source: MediaSource::Plain(uri.to_owned()), // Use owned URI
                    format: MediaFormat::File,
                };

            // Get media content
            let response = client
                .media()
                    .get_media_content(&request, true) // `true` for use_cache
                    .await
                    .map_err(MediaError::matrix_sdk)?;

                Ok(response)
            }.await;

            // Map MediaError to crate::error::Error
            result.map_err(crate::error::Error::Media)
        })
    }

    /// Download content to a file.
    pub fn download_to_file(&self, mxc_uri: &str, path: PathBuf) -> MatrixFuture<()> {
        let mxc_uri_owned = mxc_uri.to_owned(); // Clone for the async block
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let result = async {
                // Parse the MXC URI
                let uri = MxcUri::parse(&mxc_uri_owned)
                     .map_err(|e| MediaError::InvalidUri(e.to_string()))?;

                // Create a media request
                let request = MediaRequest {
                    source: MediaSource::Plain(uri.to_owned()), // Use owned URI
                    format: MediaFormat::File,
                };

            // Get media content
            let data = client
                .media()
                .get_media_content(&request, true) // `true` for use_cache
                .await
                .map_err(MediaError::matrix_sdk)?;

            // Write to file
            tokio::fs::write(path, data)
                    .await
                    .map_err(|e| MediaError::IoError(e.to_string()))?;

                Ok(())
            }.await;

            // Map MediaError to crate::error::Error
            result.map_err(crate::error::Error::Media)
        })
    }

    /// Get a thumbnail for content.
    pub fn get_thumbnail(&self, mxc_uri: &str, width: u32, height: u32) -> MatrixFuture<Vec<u8>> {
        let mxc_uri_owned = mxc_uri.to_owned(); // Clone for the async block
        let client = self.client.clone();
        let width = UInt::from(width);
        let height = UInt::from(height);

        MatrixFuture::spawn(async move {
            let result = async {
                // Parse the MXC URI
                let uri = MxcUri::parse(&mxc_uri_owned)
                     .map_err(|e| MediaError::InvalidUri(e.to_string()))?;

                // Create thumbnail settings
                let thumbnail_settings = MediaThumbnailSettings::new(width, height);

                // Create a media request
                let request = MediaRequest {
                    source: MediaSource::Plain(uri.to_owned()), // Use owned URI
                    format: MediaFormat::Thumbnail(thumbnail_settings),
                };

            // Get media content
            let data = client
                .media()
                .get_media_content(&request, true) // `true` for use_cache
                    .await
                    .map_err(MediaError::matrix_sdk)?;

                Ok(data)
            }.await;

            // Map MediaError to crate::error::Error
            result.map_err(crate::error::Error::Media)
        })
    }

    /// Get a thumbnail with specific options.
    pub fn get_thumbnail_with_options(
        &self,
        mxc_uri: &str,
        width: u32,
        height: u32,
        method: ThumbnailMethod,
    ) -> MatrixFuture<Vec<u8>> {
        let mxc_uri_owned = mxc_uri.to_owned(); // Clone for the async block
        let client = self.client.clone();
        let width = UInt::from(width);
        let height = UInt::from(height);

        MatrixFuture::spawn(async move {
            let result = async {
                // Parse the MXC URI
                let uri = MxcUri::parse(&mxc_uri_owned)
                     .map_err(|e| MediaError::InvalidUri(e.to_string()))?;

                // Create thumbnail settings with method
                let thumbnail_settings = MediaThumbnailSettings::new(width, height).method(method); // Use builder pattern

                // Create a media request
                let request = MediaRequest {
                    source: MediaSource::Plain(uri.to_owned()), // Use owned URI
                    format: MediaFormat::Thumbnail(thumbnail_settings),
                };

            // Get media content
            let data = client
                .media()
                .get_media_content(&request, true) // `true` for use_cache
                    .await
                    .map_err(MediaError::matrix_sdk)?;

                Ok(data)
            }.await;

            // Map MediaError to crate::error::Error
            result.map_err(crate::error::Error::Media)
        })
    }

    /// Get the download URL for content.
    pub fn get_download_url(&self, mxc_uri: &str) -> crate::error::Result<Option<String>> { // Return crate::error::Result
        let client = self.client.clone();
        let uri = matrix_sdk::ruma::MxcUri::parse(mxc_uri) // Use ruma::MxcUri::parse
            .map_err(|e| MediaError::InvalidUri(e.to_string()))?;
        // Check SDK 0.10+ for getting download URL, might be on client or media helper
        // Placeholder: Replace with actual SDK 0.10+ method
        warn!("get_download_url needs verification for SDK 0.10+ method");
        // Placeholder: Assume download_url exists and returns Result<String, _>
        let url = client.media().download_url(&uri).map(|u| Some(u.to_string())).map_err(MediaError::matrix_sdk)?; // Example
        // let url: Option<String> = None; // Placeholder
        Ok(url).map_err(crate::error::Error::Media)
    }

    /// Get the thumbnail URL for content.
    pub fn get_thumbnail_url(&self, mxc_uri: &str, width: u32, height: u32) -> crate::error::Result<Option<String>> { // Return crate::error::Result
        let client = self.client.clone();
        let width = UInt::from(width);
        let height = UInt::from(height);
        let thumbnail_settings = MediaThumbnailSettings::new(width, height);
        let uri = matrix_sdk::ruma::MxcUri::parse(mxc_uri) // Use ruma::MxcUri::parse
            .map_err(|e| MediaError::InvalidUri(e.to_string()))?;
        // Check SDK 0.10+ for getting thumbnail URL
        // Placeholder: Replace with actual SDK 0.10+ method
        warn!("get_thumbnail_url needs verification for SDK 0.10+ method");
        // Placeholder: Assume thumbnail_url exists and returns Result<String, _>
        let url = client.media().thumbnail_url(&uri, thumbnail_settings).map(|u| Some(u.to_string())).map_err(MediaError::matrix_sdk)?; // Example
        // let url: Option<String> = None; // Placeholder
        Ok(url).map_err(crate::error::Error::Media)
    }
}
