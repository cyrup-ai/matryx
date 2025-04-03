//! Media manager wrapper with synchronous interfaces
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Media functionality
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Handle;

use matrix_sdk::{
    media::MediaThumbnailSettings,
    ruma::{api::client::media::get_content_thumbnail::v3::Method as ThumbnailMethod, UInt},
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
    ) -> MatrixFuture<String> {
        let content_type = content_type.to_owned();
        let filename = filename.map(|s| s.to_owned());
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // Create the upload request
            let mut request =
                matrix_sdk::ruma::api::client::media::create_content::v3::Request::new(data);
            request.content_type = Some(content_type);
            if let Some(name) = filename {
                request.filename = Some(name);
            }

            // Send the request
            let response = client.send(request).await.map_err(MediaError::matrix_sdk)?;

            Ok(response.content_uri)
        })
    }

    /// Upload a file from disk to the homeserver.
    pub fn upload_file(&self, path: PathBuf) -> MatrixFuture<String> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
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
            .to_string();

            // Create the upload request
            let mut request =
                matrix_sdk::ruma::api::client::media::create_content::v3::Request::new(data);
            request.content_type = Some(content_type);
            request.filename = Some(filename.to_string());

            // Send the request
            let response = client.send(request).await.map_err(MediaError::matrix_sdk)?;

            Ok(response.content_uri)
        })
    }

    /// Download content from the homeserver.
    pub fn download(&self, mxc_uri: &str) -> MatrixFuture<Vec<u8>> {
        let mxc_uri = mxc_uri.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let source = matrix_sdk::ruma::events::room::MediaSource::Plain(mxc_uri);
            let request = matrix_sdk::media::MediaRequestParameters {
                source,
                format: matrix_sdk::media::MediaFormat::File,
            };

            let response = client
                .media()
                .get_media_content(&request, true)
                .await
                .map_err(MediaError::matrix_sdk)?;

            Ok(response)
        })
    }

    /// Download content to a file.
    pub fn download_to_file(&self, mxc_uri: &str, path: PathBuf) -> MatrixFuture<()> {
        let mxc_uri = mxc_uri.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let source = matrix_sdk::ruma::events::room::MediaSource::Plain(mxc_uri);
            let request = matrix_sdk::media::MediaRequestParameters {
                source,
                format: matrix_sdk::media::MediaFormat::File,
            };

            let data = client
                .media()
                .get_media_content(&request, true)
                .await
                .map_err(MediaError::matrix_sdk)?;

            tokio::fs::write(path, data)
                .await
                .map_err(|e| MediaError::IoError(e.to_string()))?;

            Ok(())
        })
    }

    /// Get a thumbnail for content.
    pub fn get_thumbnail(&self, mxc_uri: &str, width: u32, height: u32) -> MatrixFuture<Vec<u8>> {
        let mxc_uri = mxc_uri.to_owned();
        let client = self.client.clone();
        let width = UInt::from(width);
        let height = UInt::from(height);

        MatrixFuture::spawn(async move {
            let thumbnail_settings = MediaThumbnailSettings::new(width, height);
            let source = matrix_sdk::ruma::events::room::MediaSource::Plain(mxc_uri);
            let request = matrix_sdk::media::MediaRequestParameters {
                source,
                format: matrix_sdk::media::MediaFormat::Thumbnail(thumbnail_settings),
            };

            let data = client
                .media()
                .get_media_content(&request, true)
                .await
                .map_err(MediaError::matrix_sdk)?;

            Ok(data)
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
        let mxc_uri = mxc_uri.to_owned();
        let client = self.client.clone();
        let width = UInt::from(width);
        let height = UInt::from(height);

        MatrixFuture::spawn(async move {
            let thumbnail_settings = MediaThumbnailSettings::with_method(method, width, height);
            let source = matrix_sdk::ruma::events::room::MediaSource::Plain(mxc_uri);
            let request = matrix_sdk::media::MediaRequestParameters {
                source,
                format: matrix_sdk::media::MediaFormat::Thumbnail(thumbnail_settings),
            };

            let data = client
                .media()
                .get_media_content(&request, true)
                .await
                .map_err(MediaError::matrix_sdk)?;

            Ok(data)
        })
    }

    /// Get the download URL for content.
    pub fn get_download_url(&self, mxc_uri: &str) -> Option<String> {
        let client = self.client.clone();
        let uri = match matrix_sdk::ruma::MxcUri::parse(mxc_uri) {
            Ok(uri) => uri,
            Err(_) => return None,
        };
        client.media().get_content_download_url(&uri).map(|url| url.to_string())
    }

    /// Get the thumbnail URL for content.
    pub fn get_thumbnail_url(&self, mxc_uri: &str, width: u32, height: u32) -> Option<String> {
        let client = self.client.clone();
        let width = UInt::from(width);
        let height = UInt::from(height);
        let thumbnail_settings = MediaThumbnailSettings::new(width, height);
        let uri = match matrix_sdk::ruma::MxcUri::parse(mxc_uri) {
            Ok(uri) => uri,
            Err(_) => return None,
        };
        client
            .media()
            .get_content_thumbnail_url(&uri, thumbnail_settings)
            .map(|url| url.to_string())
    }
}
