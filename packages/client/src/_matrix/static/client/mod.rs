//! Static client configuration endpoints
//!
//! These endpoints provide cacheable, static configuration data
//! Reference: packages/server/src/_matrix/static/client/login.rs

use crate::http_client::{HttpClientError, MatrixHttpClient};
use reqwest::Method;
use serde::Deserialize;

/// Login flow type
#[derive(Debug, Clone, Deserialize)]
pub struct LoginFlow {
    /// Flow type (e.g., "m.login.password", "m.login.sso")
    #[serde(rename = "type")]
    pub flow_type: String,
}

/// Response from GET /_matrix/static/client/login
#[derive(Debug, Deserialize)]
pub struct StaticLoginResponse {
    /// Available login flows
    pub flows: Vec<LoginFlow>,
}

/// Client for static client configuration
#[derive(Clone)]
pub struct StaticClient {
    http_client: MatrixHttpClient,
}

impl StaticClient {
    pub fn new(http_client: MatrixHttpClient) -> Self {
        Self { http_client }
    }

    /// Get static login flows
    ///
    /// GET /_matrix/static/client/login
    ///
    /// Returns cacheable login flow configuration
    pub async fn get_login_flows(&self) -> Result<StaticLoginResponse, HttpClientError> {
        self.http_client
            .request(
                Method::GET,
                "/_matrix/static/client/login",
                None::<&()>,
            )
            .await
    }
}
