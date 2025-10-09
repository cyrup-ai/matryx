//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use crate::federation::event_signer::EventSigner;
use async_trait::async_trait;
use matryx_surrealdb::repository::{
    error::RepositoryError, federation_media_trait::FederationMediaClientTrait,
    media_service::MediaDownloadResult,
};
use reqwest::Response;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

#[derive(Debug, thiserror::Error)]
pub enum FederationMediaError {
    #[error("Remote server returned M_UNRECOGNIZED")]
    Unrecognized,
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("Invalid response format")]
    InvalidResponse,
    #[error("Media not found on remote server")]
    NotFound,
    #[error("Federation signing failed: {0}")]
    SigningError(String),
    #[error("Server discovery failed for {server}: {error}")]
    ServerDiscoveryError { server: String, error: String },
}

/// Federation media client for downloading media from remote Matrix servers
/// with automatic fallback to deprecated endpoints when M_UNRECOGNIZED is returned
pub struct FederationMediaClient {
    http_client: Arc<reqwest::Client>,
    event_signer: Arc<EventSigner>,
    homeserver_name: String,
    use_https: bool,
}

impl FederationMediaClient {
    /// Create a new federation media client
    pub fn new(
        http_client: Arc<reqwest::Client>,
        event_signer: Arc<EventSigner>,
        homeserver_name: String,
        use_https: bool,
    ) -> Self {
        Self { http_client, event_signer, homeserver_name, use_https }
    }

    /// Download media with automatic fallback to deprecated endpoints
    ///
    /// This implements the Matrix Content Repository fallback protocol:
    /// 1. Try new federation endpoint `/_matrix/federation/v1/media/download/{mediaId}` first
    /// 2. If 404 M_UNRECOGNIZED is returned, fallback to deprecated endpoint
    /// 3. Use `/_matrix/media/v3/download/{serverName}/{mediaId}?allow_remote=false`
    pub async fn download_media(
        &self,
        server_name: &str,
        media_id: &str,
    ) -> Result<MediaDownloadResult, FederationMediaError> {
        // Prevent attempting to download media from ourselves
        if server_name == self.homeserver_name {
            return Err(FederationMediaError::InvalidResponse);
        }

        info!(
            "Downloading media from remote server: server={}, media_id={}, homeserver={}",
            server_name, media_id, self.homeserver_name
        );

        // 1. Try new federation endpoint first
        match self.try_federation_endpoint(server_name, media_id).await {
            Ok(result) => {
                info!(
                    "Successfully downloaded media using federation endpoint: server={}, media_id={}, size={}",
                    server_name, media_id, result.content_length
                );
                Ok(result)
            },
            Err(FederationMediaError::Unrecognized) => {
                info!(
                    "Federation endpoint returned M_UNRECOGNIZED, falling back to deprecated endpoint: server={}, media_id={}",
                    server_name, media_id
                );
                // 2. Fallback to deprecated endpoint
                self.try_deprecated_endpoint(server_name, media_id).await
            },
            Err(e) => {
                error!(
                    "Federation endpoint failed with non-fallback error: server={}, media_id={}, error={}",
                    server_name, media_id, e
                );
                Err(e)
            },
        }
    }

    /// Primary attempt using new federation endpoint with X-Matrix authentication
    async fn try_federation_endpoint(
        &self,
        server_name: &str,
        media_id: &str,
    ) -> Result<MediaDownloadResult, FederationMediaError> {
        // Construct federation endpoint URL
        let protocol = if self.use_https { "https" } else { "http" };
        let url =
            format!("{}://{}/_matrix/federation/v1/media/download/{}", protocol, server_name, media_id);

        debug!("Attempting federation media download: url={}", url);

        // Create signed federation request using existing signing infrastructure
        let request_builder = self.http_client.get(&url);
        let uri = format!("/_matrix/federation/v1/media/download/{}", media_id);
        let signed_request = self
            .event_signer
            .sign_federation_request(request_builder, "GET", &uri, server_name, None)
            .await
            .map_err(|e| FederationMediaError::SigningError(e.to_string()))?;

        // Send the signed request
        let response = signed_request.send().await?;
        let status = response.status();

        debug!("Federation media request response: url={}, status={}", url, status);

        // Handle 404 responses - check for M_UNRECOGNIZED specifically
        if status == 404 {
            if self.is_unrecognized_error(response).await {
                return Err(FederationMediaError::Unrecognized);
            } else {
                return Err(FederationMediaError::NotFound);
            }
        }

        // Handle other non-success responses
        if !status.is_success() {
            warn!("Federation media request failed: url={}, status={}", url, status);
            return Err(FederationMediaError::NotFound);
        }

        // Parse successful response
        self.parse_media_response(response).await
    }

    /// Fallback using deprecated endpoint with allow_remote=false parameter
    async fn try_deprecated_endpoint(
        &self,
        server_name: &str,
        media_id: &str,
    ) -> Result<MediaDownloadResult, FederationMediaError> {
        // Construct deprecated endpoint URL with required allow_remote=false parameter
        let protocol = if self.use_https { "https" } else { "http" };
        let url = format!(
            "{}://{}/_matrix/media/v3/download/{}/{}?allow_remote=false",
            protocol, server_name, server_name, media_id
        );

        debug!("Attempting deprecated media download: url={}", url);

        // Note: Deprecated endpoints don't require federation signing per Matrix spec
        let response = self.http_client.get(&url).send().await?;
        let status = response.status();

        debug!("Deprecated media request response: url={}, status={}", url, status);

        if !status.is_success() {
            warn!("Deprecated media request failed: url={}, status={}", url, status);
            return Err(FederationMediaError::NotFound);
        }

        let result = self.parse_media_response(response).await?;
        info!(
            "Successfully downloaded media using deprecated endpoint: server={}, media_id={}, size={}",
            server_name, media_id, result.content_length
        );

        Ok(result)
    }

    /// Check if 404 response contains M_UNRECOGNIZED error code
    ///
    /// Matrix specification requires checking for specific error format:
    /// {"errcode": "M_UNRECOGNIZED", "error": "Unrecognized request"}
    async fn is_unrecognized_error(&self, response: Response) -> bool {
        match response.text().await {
            Ok(text) => {
                debug!("Checking 404 response for M_UNRECOGNIZED: body={}", text);

                match serde_json::from_str::<Value>(&text) {
                    Ok(json) => {
                        let is_unrecognized = json.get("errcode")
                            == Some(&Value::String("M_UNRECOGNIZED".to_string()));

                        if is_unrecognized {
                            debug!(
                                "Detected M_UNRECOGNIZED error, will fallback to deprecated endpoint"
                            );
                        } else {
                            debug!(
                                "404 response is not M_UNRECOGNIZED: errcode={:?}",
                                json.get("errcode")
                            );
                        }

                        is_unrecognized
                    },
                    Err(e) => {
                        warn!("Failed to parse 404 response as JSON: error={}, body={}", e, text);
                        false
                    },
                }
            },
            Err(e) => {
                warn!("Failed to read 404 response body: error={}", e);
                false
            },
        }
    }

    /// Parse successful media response into MediaDownloadResult
    async fn parse_media_response(
        &self,
        response: Response,
    ) -> Result<MediaDownloadResult, FederationMediaError> {
        // Extract content type from headers
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        // Extract content length
        let content_length = response.content_length().unwrap_or(0);

        // Extract filename from Content-Disposition header if present
        let filename = response
            .headers()
            .get("content-disposition")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| {
                // Simple filename extraction from Content-Disposition header
                // Look for filename="..." or filename*=UTF-8''...
                if let Some(start) = v.find("filename=") {
                    let filename_part = &v[start + 9..];
                    if let Some(quoted) = filename_part.strip_prefix('"') {
                        // Quoted filename
                        quoted.split('"').next().map(|s| s.to_string())
                    } else {
                        // Unquoted filename
                        filename_part.split_whitespace().next().map(|s| s.to_string())
                    }
                } else {
                    None
                }
            });

        // Read response body
        let content = response.bytes().await?.to_vec();

        debug!(
            "Successfully parsed media response: content_type={}, content_length={}, filename={:?}, actual_size={}",
            content_type,
            content_length,
            filename,
            content.len()
        );

        Ok(MediaDownloadResult { content, content_type, content_length, filename })
    }
}

// Implement the trait for dependency injection and clean architecture
#[async_trait]
impl FederationMediaClientTrait for FederationMediaClient {
    async fn download_media(
        &self,
        server_name: &str,
        media_id: &str,
    ) -> Result<MediaDownloadResult, RepositoryError> {
        self.download_media(server_name, media_id).await.map_err(|e| match e {
            FederationMediaError::NotFound => RepositoryError::NotFound {
                entity_type: "RemoteMedia".to_string(),
                id: format!("{}:{}", server_name, media_id),
            },
            FederationMediaError::Unrecognized => RepositoryError::InvalidOperation {
                reason: format!(
                    "Remote server {} does not support federation media endpoints",
                    server_name
                ),
            },
            FederationMediaError::HttpError(http_err) => RepositoryError::InvalidOperation {
                reason: format!("HTTP request failed: {}", http_err),
            },
            FederationMediaError::InvalidResponse => RepositoryError::InvalidData {
                message: "Invalid response from remote server".to_string(),
            },
            FederationMediaError::SigningError(msg) => RepositoryError::InvalidOperation {
                reason: format!("Federation signing failed: {}", msg),
            },
            FederationMediaError::ServerDiscoveryError { server, error } => {
                RepositoryError::InvalidOperation {
                    reason: format!("Server discovery failed for {}: {}", server, error),
                }
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    // Note: These are unit tests for the logic, not integration tests
    // Integration testing would require actual Matrix servers

    #[tokio::test]
    async fn test_is_unrecognized_error_with_valid_json() {
        let _client = create_test_client().await;

        // Mock response would be created here in a real test environment
        // For now, this demonstrates the test structure

        // Test case: Valid M_UNRECOGNIZED response
        let _json_body = r#"{"errcode": "M_UNRECOGNIZED", "error": "Unrecognized request"}"#;
        // Would need to create a mock Response with this body

        // Test case: Different error code
        let _json_body = r#"{"errcode": "M_NOT_FOUND", "error": "Not found"}"#;
        // Would need to create a mock Response with this body

        // Test case: Invalid JSON
        let _invalid_body = "not json";
        // Would need to create a mock Response with this body
    }

    async fn create_test_client() -> FederationMediaClient {
        use matryx_surrealdb::test_utils::create_test_database;
        use std::sync::Arc;

        let test_database = create_test_database().await.expect("Failed to create test database");
        let test_db = test_database.db.clone();

        let session_repo =
            matryx_surrealdb::repository::session::SessionRepository::new(test_db.clone());
        let key_server_repo =
            matryx_surrealdb::repository::key_server::KeyServerRepository::new(test_db.clone());

        let session_service = Arc::new(crate::auth::session_service::MatrixSessionService::new(
            b"test_secret",
            b"test_secret",
            "test.example.org".to_string(),
            session_repo,
            key_server_repo,
        ));

        let http_client = Arc::new(reqwest::Client::new());
        let well_known_client =
            Arc::new(crate::federation::well_known_client::WellKnownClient::new(http_client.clone(), true));
        let dns_resolver = Arc::new(
            crate::federation::dns_resolver::MatrixDnsResolver::new(well_known_client, true)
                .expect("Failed to create DNS resolver"),
        );

        let event_signer = Arc::new(
            EventSigner::new(
                session_service,
                test_db,
                dns_resolver,
                "test.example.org".to_string(),
                "ed25519:test".to_string(),
            )
            .expect("Failed to create event signer"),
        );

        FederationMediaClient::new(http_client, event_signer, "test.example.org".to_string(), true)
    }
}
