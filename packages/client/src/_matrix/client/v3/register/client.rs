//! Registration client implementation using MatrixHttpClient

use crate::http_client::{MatrixHttpClient, HttpClientError};
use super::{RegisterRequest, RegisterResponse, RegistrationFlowsResponse};

/// Client for Matrix registration operations
pub struct RegisterClient {
    http_client: MatrixHttpClient,
}

impl RegisterClient {
    /// Create new registration client
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Get available registration flows
    ///
    /// Queries GET /_matrix/client/v3/register to discover what
    /// authentication stages are required on this homeserver.
    ///
    /// # Returns
    /// - `Ok(RegistrationFlowsResponse)` with available flows
    /// - `Err(HttpClientError)` on network/server errors
    pub async fn get_registration_flows(&self) -> Result<RegistrationFlowsResponse, HttpClientError> {
        self.http_client
            .get("/_matrix/client/v3/register")
            .await
    }

    /// Register a new user account
    ///
    /// Sends POST /_matrix/client/v3/register with account details.
    ///
    /// # Arguments
    /// * `request` - Registration request with username, password, and auth data
    ///
    /// # Returns
    /// - `Ok(RegisterResponse)` with user_id and optional access_token
    /// - `Err(HttpClientError::Matrix)` with specific error codes:
    ///   - M_USER_IN_USE (400) - Username already taken
    ///   - M_INVALID_USERNAME (400) - Username format invalid
    ///   - M_WEAK_PASSWORD (400) - Password doesn't meet requirements
    ///   - M_UNAUTHORIZED (401) - Additional auth stages required
    ///
    /// # Multi-Stage Authentication
    ///
    /// If server returns 401, the response will contain `flows` indicating
    /// required auth stages (CAPTCHA, email verification, etc.). Client must
    /// complete these stages and retry with `auth` field populated.
    pub async fn register(&self, request: &RegisterRequest) -> Result<RegisterResponse, HttpClientError> {
        let response: RegisterResponse = self.http_client
            .post("/_matrix/client/v3/register", request)
            .await?;

        // If access_token was returned (inhibit_login=false), set it for future requests
        if let Some(ref token) = response.access_token {
            self.http_client.set_access_token(token.clone()).await;
        }

        Ok(response)
    }

    /// Register with username and password (convenience method)
    ///
    /// Creates a basic registration request and submits it.
    ///
    /// # Arguments
    /// * `username` - Desired username (localpart only)
    /// * `password` - Account password
    /// * `device_display_name` - Optional human-readable device name
    ///
    /// # Note
    /// This may fail with M_UNAUTHORIZED if server requires additional
    /// auth stages (CAPTCHA, email verification). Use `register()` with
    /// full `RegisterRequest` including `auth` field for multi-stage flows.
    pub async fn register_with_password(
        &self,
        username: &str,
        password: &str,
        device_display_name: Option<String>,
    ) -> Result<RegisterResponse, HttpClientError> {
        let mut request = RegisterRequest::new(username, password);
        
        if let Some(name) = device_display_name {
            request = request.with_display_name(name);
        }

        self.register(&request).await
    }
}
