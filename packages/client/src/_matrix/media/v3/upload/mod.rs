//! Matrix Media Upload Client (v3)
//!
//! Implements POST /_matrix/media/v3/upload for binary media uploads

use crate::http_client::{HttpClientError, MatrixHttpClient};
use serde::{Deserialize, Serialize};
use reqwest::Method;

/// Response from media upload endpoint
#[derive(Debug, Serialize, Deserialize)]
pub struct MediaUploadResponse {
    pub content_uri: String,
}

/// Response from media download endpoint
#[derive(Debug)]
pub struct MediaDownloadResponse {
    pub content_type: String,
    pub data: Vec<u8>,
    pub filename: Option<String>,
}

/// Client for Matrix media operations
pub struct MediaClient {
    http_client: MatrixHttpClient,
    reqwest_client: reqwest::Client,
}

impl MediaClient {
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { 
            http_client,
            reqwest_client: reqwest::Client::new(),
        }
    }

    /// Upload media to homeserver
    ///
    /// # Arguments
    /// * `content_type` - MIME type (e.g., "image/png")
    /// * `filename` - Optional filename for upload
    /// * `data` - Binary media data
    ///
    /// # Returns
    /// * `Result<MediaUploadResponse, HttpClientError>` - Content URI or error
    pub async fn upload_media(
        &self,
        content_type: &str,
        filename: Option<&str>,
        data: Vec<u8>,
    ) -> Result<MediaUploadResponse, HttpClientError> {
        // 1. Build URL with optional filename query parameter
        let mut path = "/_matrix/media/v3/upload".to_string();
        if let Some(name) = filename {
            let encoded = urlencoding::encode(name);
            path.push_str(&format!("?filename={}", encoded));
        }

        // 2. Get access token (required for upload)
        let token = self.http_client.get_access_token().await?;

        // 3. Build full URL
        let url = self.http_client.homeserver_url().join(&path)?;

        // 4. Build request using stored client
        let response = self.reqwest_client
            .request(Method::POST, url)
            .bearer_auth(token)
            .header("Content-Type", content_type)
            .body(data)
            .send()
            .await?;

        // 5. Handle response
        let status = response.status();
        if status.is_success() {
            let upload_response = response.json::<MediaUploadResponse>().await?;
            Ok(upload_response)
        } else {
            let error_body = response.text().await?;
            Err(HttpClientError::Matrix {
                status: status.as_u16(),
                errcode: "M_UNKNOWN".to_string(),
                error: error_body,
                retry_after_ms: None,
            })
        }
    }

    /// Download media from homeserver
    ///
    /// # Arguments
    /// * `server_name` - Server hosting the media
    /// * `media_id` - Media identifier
    ///
    /// # Returns
    /// * `Result<MediaDownloadResponse, HttpClientError>` - Media data or error
    pub async fn download_media(
        &self,
        server_name: &str,
        media_id: &str,
    ) -> Result<MediaDownloadResponse, HttpClientError> {
        // 1. Build URL
        let path = format!("/_matrix/media/v3/download/{}/{}", server_name, media_id);
        let url = self.http_client.homeserver_url().join(&path)?;

        // 2. Download does NOT require authentication
        let response = self.reqwest_client.get(url).send().await?;

        // 3. Check status and extract headers before consuming response
        let status = response.status();
        let headers = response.headers().clone();
        
        if !status.is_success() {
            let error_body = response.text().await?;
            return Err(HttpClientError::Matrix {
                status: status.as_u16(),
                errcode: "M_NOT_FOUND".to_string(),
                error: error_body,
                retry_after_ms: None,
            });
        }

        // 4. Extract content-type from headers
        let content_type = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        // 5. Extract filename from Content-Disposition header
        let filename = headers
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| {
                s.split("filename=")
                    .nth(1)
                    .map(|f| f.trim_matches('"').to_string())
            });

        // 6. Get binary data
        let data = response.bytes().await?.to_vec();

        Ok(MediaDownloadResponse {
            content_type,
            data,
            filename,
        })
    }

    /// Get thumbnail of media
    ///
    /// # Arguments
    /// * `server_name` - Server hosting the media
    /// * `media_id` - Media identifier
    /// * `width` - Desired width in pixels
    /// * `height` - Desired height in pixels
    /// * `method` - Resize method: "crop" or "scale"
    ///
    /// # Returns
    /// * `Result<MediaDownloadResponse, HttpClientError>` - Thumbnail data or error
    pub async fn get_thumbnail(
        &self,
        server_name: &str,
        media_id: &str,
        width: u32,
        height: u32,
        method: &str,
    ) -> Result<MediaDownloadResponse, HttpClientError> {
        // 1. Build URL with query parameters
        let path = format!(
            "/_matrix/media/v3/thumbnail/{}/{}?width={}&height={}&method={}",
            server_name, media_id, width, height, method
        );
        let url = self.http_client.homeserver_url().join(&path)?;

        // 2. Thumbnail does NOT require authentication
        let response = self.reqwest_client.get(url).send().await?;

        // 3. Check status and extract headers before consuming response
        let status = response.status();
        let headers = response.headers().clone();
        
        if !status.is_success() {
            let error_body = response.text().await?;
            return Err(HttpClientError::Matrix {
                status: status.as_u16(),
                errcode: "M_NOT_FOUND".to_string(),
                error: error_body,
                retry_after_ms: None,
            });
        }

        // 4. Extract content-type
        let content_type = headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        // 5. Get binary data
        let data = response.bytes().await?.to_vec();

        Ok(MediaDownloadResponse {
            content_type,
            data,
            filename: None,
        })
    }
}
