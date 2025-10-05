//! Low-level HTTP client infrastructure for Matrix API requests
//!
//! This module provides foundational HTTP client functionality with:
//! - Generic request/response handling
//! - Matrix-spec-compliant error parsing
//! - Retry logic with exponential backoff
//! - Thread-safe authentication token management

use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use url::Url;

/// HTTP client errors with Matrix-spec error handling
#[derive(Debug, thiserror::Error)]
pub enum HttpClientError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Matrix error {errcode}: {error} (HTTP {status})")]
    Matrix {
        status: u16,
        errcode: String,
        error: String,
        retry_after_ms: Option<u64>,
    },

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("Authentication required")]
    AuthenticationRequired,

    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
}

/// Matrix error response format per specification
#[derive(Debug, Deserialize)]
struct MatrixErrorResponse {
    errcode: String,
    error: String,
    #[serde(default)]
    retry_after_ms: Option<u64>,
    #[serde(default)]
    #[allow(dead_code)] // Used for deserialization but not accessed
    soft_logout: Option<bool>,
}
/// Low-level HTTP client for Matrix API requests
#[derive(Clone)]
pub struct MatrixHttpClient {
    client: Client,
    homeserver_url: Url,
    access_token: Arc<RwLock<Option<String>>>,
}

impl MatrixHttpClient {
    /// Create a new Matrix HTTP client
    ///
    /// # Arguments
    /// * `homeserver_url` - Base URL of the Matrix homeserver
    ///
    /// # Returns
    /// * `Result<Self, HttpClientError>` - New client or error
    pub fn new(homeserver_url: Url) -> Result<Self, HttpClientError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .user_agent("MaxTryX Client/1.0")
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;

        Ok(Self {
            client,
            homeserver_url,
            access_token: Arc::new(RwLock::new(None)),
        })
    }

    /// Get the homeserver URL
    pub fn homeserver_url(&self) -> &Url {
        &self.homeserver_url
    }

    /// Generic request method for any request/response types
    ///
    /// # Arguments
    /// * `method` - HTTP method (GET, POST, PUT, DELETE, etc.)
    /// * `path` - API path (e.g., "/_matrix/client/v3/login")
    /// * `body` - Optional request body
    ///
    /// # Returns
    /// * `Result<R, HttpClientError>` - Deserialized response or error
    pub async fn request<T, R>(
        &self,
        method: Method,
        path: &str,
        body: Option<&T>,
    ) -> Result<R, HttpClientError>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        // 1. Construct full URL
        let url = self.homeserver_url.join(path)?;

        // 2. Build request
        let mut req = self.client.request(method, url);

        // 3. Add authorization if token present
        if let Some(token) = self.access_token.read().await.as_ref() {
            req = req.bearer_auth(token);
        }

        // 4. Add JSON body if provided
        if let Some(body) = body {
            req = req.json(body);
        }

        // 5. Send request
        let response = req.send().await?;
        let status = response.status();

        // 6. Handle response
        if status.is_success() {
            let data = response.json::<R>().await?;
            Ok(data)
        } else {
            // Parse Matrix error response
            let error_body = response.text().await?;
            self.parse_matrix_error(status.as_u16(), &error_body)
        }
    }

    /// Parse Matrix error response per specification
    fn parse_matrix_error<T>(&self, status: u16, body: &str) -> Result<T, HttpClientError> {
        match serde_json::from_str::<MatrixErrorResponse>(body) {
            Ok(matrix_err) => Err(HttpClientError::Matrix {
                status,
                errcode: matrix_err.errcode,
                error: matrix_err.error,
                retry_after_ms: matrix_err.retry_after_ms,
            }),
            Err(_) => {
                // Fallback: non-JSON error response
                Err(HttpClientError::Matrix {
                    status,
                    errcode: "M_UNKNOWN".to_string(),
                    error: body.to_string(),
                    retry_after_ms: None,
                })
            }
        }
    }

    /// Set access token for authenticated requests
    ///
    /// # Arguments
    /// * `token` - Access token from login/registration
    pub async fn set_access_token(&self, token: String) {
        let mut guard = self.access_token.write().await;
        *guard = Some(token);
    }

    /// Get current access token
    ///
    /// # Returns
    /// * `Result<String, HttpClientError>` - Token or authentication error
    pub async fn get_access_token(&self) -> Result<String, HttpClientError> {
        self.access_token
            .read()
            .await
            .clone()
            .ok_or(HttpClientError::AuthenticationRequired)
    }

    /// Clear access token (logout)
    pub async fn clear_access_token(&self) {
        let mut guard = self.access_token.write().await;
        *guard = None;
    }

    /// Check if access token is set
    pub async fn has_access_token(&self) -> bool {
        self.access_token.read().await.is_some()
    }

    /// Request with retry logic and exponential backoff
    ///
    /// # Arguments
    /// * `method` - HTTP method
    /// * `path` - API path
    /// * `body` - Optional request body
    /// * `max_retries` - Maximum number of retry attempts
    ///
    /// # Returns
    /// * `Result<R, HttpClientError>` - Response or error after retries
    pub async fn request_with_retry<T, R>(
        &self,
        method: Method,
        path: &str,
        body: Option<&T>,
        max_retries: u32,
    ) -> Result<R, HttpClientError>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        let mut attempt = 0;

        loop {
            match self.request(method.clone(), path, body).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    attempt += 1;

                    // Check if we should retry
                    let should_retry = match &e {
                        // Retry on network errors
                        HttpClientError::Network(_) => true,
                        // Retry on 5xx server errors or rate limit
                        HttpClientError::Matrix { status, errcode, .. } => {
                            *status >= 500 || errcode == "M_LIMIT_EXCEEDED"
                        }
                        // Don't retry on 4xx client errors (except rate limit)
                        _ => false,
                    };

                    if !should_retry || attempt >= max_retries {
                        return Err(if attempt >= max_retries {
                            HttpClientError::MaxRetriesExceeded
                        } else {
                            e
                        });
                    }

                    // Exponential backoff: 100ms * 2^(attempt-1)
                    let delay_ms = 100 * 2u64.pow(attempt - 1);

                    // If rate limited, use server's retry_after if available
                    let delay = if let HttpClientError::Matrix {
                        retry_after_ms: Some(ms),
                        ..
                    } = &e
                    {
                        Duration::from_millis(*ms)
                    } else {
                        Duration::from_millis(delay_ms)
                    };

                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// Convenience method for GET requests
    pub async fn get<R>(&self, path: &str) -> Result<R, HttpClientError>
    where
        R: for<'de> Deserialize<'de>,
    {
        self.request::<(), R>(Method::GET, path, None).await
    }

    /// Convenience method for POST requests
    pub async fn post<T, R>(&self, path: &str, body: &T) -> Result<R, HttpClientError>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        self.request(Method::POST, path, Some(body)).await
    }

    /// Convenience method for PUT requests
    pub async fn put<T, R>(&self, path: &str, body: &T) -> Result<R, HttpClientError>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        self.request(Method::PUT, path, Some(body)).await
    }

    /// Convenience method for DELETE requests
    pub async fn delete<R>(&self, path: &str) -> Result<R, HttpClientError>
    where
        R: for<'de> Deserialize<'de>,
    {
        self.request::<(), R>(Method::DELETE, path, None).await
    }
}
