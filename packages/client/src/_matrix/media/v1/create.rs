//! Matrix Media Upload Client (v1 - Deprecated)
//!
//! Implements POST /_matrix/media/v1/upload (deprecated Matrix spec endpoint)
//!
//! This endpoint implements the legacy Matrix media upload API that was deprecated
//! in Matrix 1.11 (MSC3916) in favor of authenticated v3 endpoints.
//!
//! **For new code, use `MediaClient::upload_media()` from `v3::upload` instead.**
//!
//! This client function exists for interoperability with older Matrix homeservers
//! that haven't migrated to v3 authenticated media endpoints. It should only be
//! used when connecting to homeservers that don't support v3 media APIs.
//!
//! ## References
//! - [MSC3916: Authenticated Media](https://github.com/matrix-org/matrix-spec-proposals/pull/3916)
//! - [Matrix 1.11 Changelog](https://spec.matrix.org/v1.11/changelog/#deprecated-endpoints)

use crate::http_client::{HttpClientError, MatrixHttpClient};
use crate::_matrix::media::v3::upload::MediaUploadResponse;

/// Legacy media upload using v1 endpoint
pub async fn upload_media_v1(
    http_client: &MatrixHttpClient,
    reqwest_client: &reqwest::Client,
    content_type: &str,
    filename: Option<&str>,
    data: Vec<u8>,
) -> Result<MediaUploadResponse, HttpClientError> {
    // 1. Build v1 URL
    let mut path = "/_matrix/media/v1/upload".to_string();
    if let Some(name) = filename {
        let encoded = urlencoding::encode(name);
        path.push_str(&format!("?filename={}", encoded));
    }

    // 2. Get access token
    let token = http_client.get_access_token().await?;

    // 3. Build full URL
    let url = http_client.homeserver_url().join(&path)?;

    // 4. Make request using passed client
    let response = reqwest_client
        .post(url)
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
