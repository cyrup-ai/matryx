//! Media manager wrapper with synchronous interfaces
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Media functionality
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Handle;
use mime;


use matrix_sdk::{
    media::MediaFormat,
    ruma::{
        OwnedMxcUri,
        UInt,
    },
    Client as MatrixClient,
};

// Media types from the SDK
use matrix_sdk::ruma::api::client::media::get_content_thumbnail::v3::Method as ThumbnailMethod;

// Define our own wrapper types based on the SDK API
struct MediaRequestConfig {
    format: MediaFormat,
}

impl MediaRequestConfig {
    fn new() -> Self {
        Self {
            format: MediaFormat::File,
        }
    }
    
    fn format(mut self, format: MediaFormat) -> Self {
        self.format = format;
        self
    }
}

struct ThumbnailSize {
    width: UInt,
    height: UInt,
    method: Option<ThumbnailMethod>,
}

impl ThumbnailSize {
    fn new(width: UInt, height: UInt) -> Self {
        Self {
            width,
            height,
            method: None,
        }
    }
    
    fn method(mut self, method: ThumbnailMethod) -> Self {
        self.method = Some(method);
        self
    }
}

use crate::error::MediaError;
use crate::future::MatrixFuture;

/// A synchronous wrapper around the Matrix SDK Media functionality.
///
/// This wrapper enables using the Media manager with a synchronous interface,
/// hiding all async complexity behind MatrixFuture objects that properly
/// implement the Future trait.
pub struct MatrixMedia {
    client: Arc<MatrixClient>,
    runtime_handle: Handle,
}

impl MatrixMedia {
    /// Create a new MatrixMedia with the provided Matrix client.
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
            let result = async {
                let content_type = content_type.parse().map_err(|_| MediaError::InvalidParameter("Invalid content type".into()))?; // Parse content type
                
                // Use the Matrix SDK 0.13 media upload API
                let mut upload_request = client.media().upload(&content_type, data, None);
                if let Some(name) = filename.as_deref() {
                    upload_request = upload_request.file_name(name);
                }

                let response = upload_request.await.map_err(MediaError::matrix_sdk)?;
                Ok(response.content_uri)
            }.await;

            result.map_err(crate::error::Error::Media)
        })
    }

    /// Upload a file from disk to the homeserver.
    pub fn upload_file(&self, path: PathBuf) -> MatrixFuture<OwnedMxcUri> { // Return OwnedMxcUri
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let result = async {
                // Read file content
                let data = tokio::fs::read(&path).await
                    .map_err(|e| MediaError::IoError(e.to_string()))?;

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

            // Use the Matrix SDK 0.13 media upload API
            let mut upload_request = client.media().upload(&content_type, data, None);
            upload_request = upload_request.file_name(&filename);

            let response = upload_request.await.map_err(MediaError::matrix_sdk)?;
            Ok(response.content_uri)
            }.await; // Await the inner async block

            // Map MediaError to crate::error::Error inside the outer future
            result.map_err(crate::error::Error::Media)
        })
    }

    /// Download content from the homeserver.
    pub fn download(&self, mxc_uri: &str) -> MatrixFuture<Vec<u8>> {
        let mxc_uri_owned = mxc_uri.to_owned(); // Clone for the async block
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let result = async {
                // Parse the MXC URI - in Matrix SDK 0.13, MxcUri is parsed differently
                let uri: matrix_sdk::ruma::OwnedMxcUri = mxc_uri_owned.try_into()
                     .map_err(|e| MediaError::InvalidUri(format!("Invalid MXC URI: {:?}", e)))?;

                // Create MediaSource from the parsed URI
                let media_source = matrix_sdk::ruma::events::room::MediaSource::Plain(uri);
                let request = matrix_sdk::media::MediaRequestParameters::from_source(&media_source);

                // Download media content using the new API
                let response = client
                    .media()
                    .get_media_content(&request, true)
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
                // Parse the MXC URI - in Matrix SDK 0.13, MxcUri is parsed differently
                let uri: matrix_sdk::ruma::OwnedMxcUri = mxc_uri_owned.try_into()
                     .map_err(|e| MediaError::InvalidUri(format!("Invalid MXC URI: {:?}", e)))?;

                // Create MediaSource and get content
                let media_source = matrix_sdk::ruma::events::room::MediaSource::Plain(uri);
                let request = matrix_sdk::media::MediaRequestParameters::from_source(&media_source);

                // Download media content using the new API
                let data = client
                    .media()
                    .get_media_content(&request, true)
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
                // Parse the MXC URI - in Matrix SDK 0.13, MxcUri is parsed differently
                let uri: matrix_sdk::ruma::OwnedMxcUri = mxc_uri_owned.try_into()
                     .map_err(|e| MediaError::InvalidUri(format!("Invalid MXC URI: {:?}", e)))?;

                // Create MediaSource and thumbnail settings
                let media_source = matrix_sdk::ruma::events::room::MediaSource::Plain(uri);
                let thumbnail_settings = matrix_sdk::media::MediaThumbnailSettings::new(
                    matrix_sdk::ruma::UInt::from(width), 
                    matrix_sdk::ruma::UInt::from(height)
                );
                let request = matrix_sdk::media::MediaRequestParameters::from_source(&media_source)
                    .with_thumbnail(thumbnail_settings);

                // Download thumbnail using the new API
                let data = client
                    .media()
                    .get_media_content(&request, true)
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
                // Parse the MXC URI - in Matrix SDK 0.13, MxcUri is parsed differently
                let uri: matrix_sdk::ruma::OwnedMxcUri = mxc_uri_owned.try_into()
                     .map_err(|e| MediaError::InvalidUri(format!("Invalid MXC URI: {:?}", e)))?;

                // Create MediaSource and thumbnail settings with method
                let media_source = matrix_sdk::ruma::events::room::MediaSource::Plain(uri);
                let thumbnail_settings = matrix_sdk::media::MediaThumbnailSettings::new(
                    matrix_sdk::ruma::UInt::from(width), 
                    matrix_sdk::ruma::UInt::from(height)
                ).method(method);
                let request = matrix_sdk::media::MediaRequestParameters::from_source(&media_source)
                    .with_thumbnail(thumbnail_settings);

                // Download thumbnail using the new API
                let data = client
                    .media()
                    .get_media_content(&request, true)
                    .await
                    .map_err(MediaError::matrix_sdk)?;

                Ok(data)
            }.await;

            // Map MediaError to crate::error::Error
            result.map_err(crate::error::Error::Media)
        })
    }

    /// Get the download URL for content.
    pub fn get_download_url(&self, mxc_uri: &str) -> MatrixFuture<Option<String>> {
        let mxc_uri_owned = mxc_uri.to_owned();
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            let result = async {
                // Parse the MXC URI - in Matrix SDK 0.13, MxcUri is parsed differently
                let uri: matrix_sdk::ruma::OwnedMxcUri = mxc_uri_owned.try_into()
                     .map_err(|e| MediaError::InvalidUri(format!("Invalid MXC URI: {:?}", e)))?;

                // Create MediaSource
                let media_source = matrix_sdk::ruma::events::room::MediaSource::Plain(uri);
                let request = matrix_sdk::media::MediaRequestParameters::from_source(&media_source);

                // Get download URL using Matrix SDK 0.13 API
                let url = client.media().get_download_url(&request);
                
                // Convert URL to string if present
                Ok(url.map(|u| u.to_string()))
            }.await;

            result.map_err(crate::error::Error::Media)
        })
    }


    /// Get the thumbnail URL for content.
    pub fn get_thumbnail_url(&self, mxc_uri: &str, width: u32, height: u32) -> MatrixFuture<Option<String>> {
        let mxc_uri_owned = mxc_uri.to_owned();
        let client = self.client.clone();
        let width = UInt::from(width);
        let height = UInt::from(height);
        let thumbnail = ThumbnailSize::new(width, height);
        
        MatrixFuture::spawn(async move {
            let result = async {
                // Parse the MXC URI - in Matrix SDK 0.13, MxcUri is parsed differently
                let uri: matrix_sdk::ruma::OwnedMxcUri = mxc_uri_owned.try_into()
                     .map_err(|e| MediaError::InvalidUri(format!("Invalid MXC URI: {:?}", e)))?;

                // Create MediaSource and thumbnail settings
                let media_source = matrix_sdk::ruma::events::room::MediaSource::Plain(uri);
                let thumbnail_settings = matrix_sdk::media::MediaThumbnailSettings::new(
                    matrix_sdk::ruma::UInt::from(width), 
                    matrix_sdk::ruma::UInt::from(height)
                );
                let request = matrix_sdk::media::MediaRequestParameters::from_source(&media_source)
                    .with_thumbnail(thumbnail_settings);
                    
                // Get thumbnail URL using Matrix SDK 0.13 API
                let url = client.media().get_download_url(&request);
                
                // Convert URL to string if present
                Ok(url.map(|u| u.to_string()))
            }.await;
            
            result.map_err(crate::error::Error::Media)
        })
    }
}
