//! Matrix client registration API
//!
//! Implements POST /_matrix/client/v3/register per Matrix specification.
//!
//! Reference: ../../../server/src/_matrix/client/v3/register/handlers.rs

pub mod client;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use client::RegisterClient;

/// Registration request body matching server-side RegistrationRequest
#[derive(Debug, Clone, Serialize)]
pub struct RegisterRequest {
    /// Desired username (localpart only, without @user:server)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Password for the account
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// Device ID (optional, server generates if not provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// Initial device display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_device_display_name: Option<String>,

    /// If true, don't automatically log in (no access_token returned)
    #[serde(default)]
    pub inhibit_login: bool,

    /// Whether client supports refresh tokens
    #[serde(default)]
    pub refresh_token: bool,

    /// User-Interactive Authentication data (for CAPTCHA, email verification, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<Value>,
}

/// Registration response body matching server-side RegistrationResponse
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterResponse {
    /// The fully-qualified Matrix user ID (MXID) created
    pub user_id: String,

    /// Access token (None if inhibit_login was true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,

    /// Device ID for this session
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,

    /// Refresh token (if requested and supported)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Access token lifetime in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_ms: Option<i64>,

    /// Well-known client configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub well_known: Option<Value>,
}

/// Available registration flows (from GET /_matrix/client/v3/register)
#[derive(Debug, Clone, Deserialize)]
pub struct RegistrationFlowsResponse {
    pub flows: Vec<RegistrationFlow>,
}

/// Single registration flow describing required auth stages
#[derive(Debug, Clone, Deserialize)]
pub struct RegistrationFlow {
    /// Ordered list of authentication stages
    pub stages: Vec<String>,
}

impl RegisterRequest {
    /// Create basic registration request with username and password
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: Some(username.into()),
            password: Some(password.into()),
            device_id: None,
            initial_device_display_name: None,
            inhibit_login: false,
            refresh_token: false,
            auth: None,
        }
    }

    /// Add User-Interactive Authentication data
    pub fn with_auth(mut self, auth: Value) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Set device ID
    pub fn with_device_id(mut self, device_id: impl Into<String>) -> Self {
        self.device_id = Some(device_id.into());
        self
    }

    /// Set initial device display name
    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.initial_device_display_name = Some(name.into());
        self
    }

    /// Don't automatically log in after registration
    pub fn inhibit_login(mut self) -> Self {
        self.inhibit_login = true;
        self
    }

    /// Request refresh token support
    pub fn with_refresh_token(mut self) -> Self {
        self.refresh_token = true;
        self
    }
}
