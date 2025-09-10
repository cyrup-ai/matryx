use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionsResponse {
    pub versions: Vec<String>,
    pub unstable_features: Option<HashMap<String, bool>>,
}

#[derive(Debug, Clone)]
pub struct MatrixVersionsClient {
    client: reqwest::Client,
    timeout: Duration,
}

impl MatrixVersionsClient {
    /// Create a new Matrix versions client with default configuration
    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("MatrixClient/1.0")
                .build()?,
            timeout: Duration::from_secs(30),
        })
    }

    /// Create a new Matrix versions client with custom timeout
    pub fn with_timeout(
        timeout: Duration,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(timeout)
                .user_agent("MatrixClient/1.0")
                .build()?,
            timeout,
        })
    }

    /// Get supported Matrix specification versions from a homeserver
    ///
    /// # Arguments
    /// * `homeserver_url` - The base URL of the Matrix homeserver (e.g., "https://matrix.org")
    ///
    /// # Returns
    /// * `Ok(VersionsResponse)` - The versions and unstable features supported by the server
    /// * `Err(Box<dyn std::error::Error + Send + Sync>)` - HTTP or parsing error
    ///
    /// # Example
    /// ```rust
    /// let client = MatrixVersionsClient::new();
    /// let versions = client.get_supported_versions("https://matrix.org").await?;
    /// println!("Supported versions: {:?}", versions.versions);
    /// ```
    pub async fn get_supported_versions(
        &self,
        homeserver_url: &str,
    ) -> Result<VersionsResponse, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/_matrix/client/versions", homeserver_url.trim_end_matches('/'));

        let response = self.client.get(&url).timeout(self.timeout).send().await?;

        // Check for HTTP errors
        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        // Parse JSON response
        let versions: VersionsResponse = response.json().await?;
        Ok(versions)
    }

    /// Check if a specific Matrix specification version is supported
    ///
    /// # Arguments
    /// * `homeserver_url` - The base URL of the Matrix homeserver
    /// * `version` - The Matrix spec version to check (e.g., "r0.6.1", "v1.1", "v1.2")
    ///
    /// # Returns
    /// * `Ok(true)` - Version is supported
    /// * `Ok(false)` - Version is not supported
    /// * `Err(...)` - Network or parsing error
    pub async fn is_version_supported(
        &self,
        homeserver_url: &str,
        version: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let versions = self.get_supported_versions(homeserver_url).await?;
        Ok(versions.versions.contains(&version.to_string()))
    }

    /// Check if a specific unstable feature is supported and enabled
    ///
    /// # Arguments
    /// * `homeserver_url` - The base URL of the Matrix homeserver
    /// * `feature` - The unstable feature identifier (e.g., "org.matrix.msc2716")
    ///
    /// # Returns
    /// * `Ok(Some(true))` - Feature is supported and enabled
    /// * `Ok(Some(false))` - Feature is supported but disabled
    /// * `Ok(None)` - Feature is not listed/supported
    /// * `Err(...)` - Network or parsing error
    pub async fn is_unstable_feature_supported(
        &self,
        homeserver_url: &str,
        feature: &str,
    ) -> Result<Option<bool>, Box<dyn std::error::Error + Send + Sync>> {
        let versions = self.get_supported_versions(homeserver_url).await?;
        Ok(versions
            .unstable_features
            .and_then(|features| features.get(feature).copied()))
    }

    /// Get raw JSON response from the versions endpoint
    ///
    /// This method returns the raw JSON Value for cases where you need
    /// access to fields not captured in the VersionsResponse struct.
    pub async fn get_versions_raw(
        &self,
        homeserver_url: &str,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/_matrix/client/versions", homeserver_url.trim_end_matches('/'));

        let response = self.client.get(&url).timeout(self.timeout).send().await?;

        // Check for HTTP errors
        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        // Parse as raw JSON
        let json: Value = response.json().await?;
        Ok(json)
    }
}

impl Default for MatrixVersionsClient {
    fn default() -> Self {
        // Use a safe fallback configuration that cannot panic
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .user_agent("MatrixClient/1.0")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            timeout: Duration::from_secs(30),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let client = MatrixVersionsClient::new().expect("Failed to create client");
        assert!(client.timeout.as_secs() == 30);
    }

    #[tokio::test]
    async fn test_client_with_timeout() {
        let client = MatrixVersionsClient::with_timeout(Duration::from_secs(60))
            .expect("Failed to create client with timeout");
        assert!(client.timeout.as_secs() == 60);
    }

    // Note: Integration tests would require a running Matrix server
    // and should be placed in tests/ directory for optional execution
}
