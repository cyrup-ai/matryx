pub mod client;

pub use client::LoginClient;

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, info};
use url::Url;

/// Login request for Matrix authentication
#[derive(Debug, Clone, Serialize)]
pub struct LoginRequest {
    #[serde(rename = "type")]
    pub login_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_device_display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

/// Login response from Matrix server
#[derive(Debug, Clone, Deserialize)]
pub struct LoginResponse {
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
    pub refresh_token: Option<String>,
    pub expires_in_ms: Option<u64>,
    pub well_known: Option<Value>,
}

/// Available login flows from Matrix server
#[derive(Debug, Clone, Deserialize)]
pub struct LoginFlowsResponse {
    pub flows: Vec<LoginFlow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginFlow {
    #[serde(rename = "type")]
    pub flow_type: String,
    pub identity_providers: Option<Vec<Value>>,
}

/// GET /_matrix/client/v3/login - Get available login flows
pub async fn get_login_flows(client: &Client, homeserver_url: &Url) -> Result<LoginFlowsResponse> {
    let url = homeserver_url.join("/_matrix/client/v3/login")?;

    debug!("Fetching login flows from: {}", url);

    let response = client.get(url).header("User-Agent", "Matryx-Client/0.1.0").send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        error!("Failed to get login flows: {} - {}", status, error_text);
        return Err(anyhow::anyhow!("Login flows request failed: {} - {}", status, error_text));
    }

    let flows = response.json::<LoginFlowsResponse>().await?;

    info!("Retrieved {} login flows from server", flows.flows.len());
    Ok(flows)
}

/// POST /_matrix/client/v3/login - Authenticate with Matrix server
pub async fn login(
    client: &Client,
    homeserver_url: &Url,
    request: LoginRequest,
) -> Result<LoginResponse> {
    let url = homeserver_url.join("/_matrix/client/v3/login")?;

    debug!("Attempting login with type: {}", request.login_type);

    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("User-Agent", "Matryx-Client/0.1.0")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_default();

        // Parse Matrix error response if possible
        if let Ok(matrix_error) = serde_json::from_str::<Value>(&error_body) &&
            let Some(errcode) = matrix_error.get("errcode").and_then(|v| v.as_str())
        {
            let error_msg = matrix_error
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");

            error!("Login failed with Matrix error {}: {}", errcode, error_msg);
            return Err(anyhow::anyhow!("Login failed: {} ({})", error_msg, errcode));
        }

        error!("Login request failed: {} - {}", status, error_body);
        return Err(anyhow::anyhow!("Login request failed: {} - {}", status, error_body));
    }

    let login_response = response.json::<LoginResponse>().await?;

    info!("Login successful for user: {}", login_response.user_id);
    Ok(login_response)
}

/// Login with username and password
pub async fn login_with_password(
    client: &Client,
    homeserver_url: &Url,
    username: &str,
    password: &str,
    device_id: Option<String>,
    device_display_name: Option<String>,
) -> Result<LoginResponse> {
    let request = LoginRequest {
        login_type: "m.login.password".to_string(),
        user: Some(username.to_string()),
        password: Some(password.to_string()),
        device_id,
        initial_device_display_name: device_display_name,
        token: None,
        refresh_token: None,
    };

    login(client, homeserver_url, request).await
}

/// Login with token (SSO, application service, etc.)
pub async fn login_with_token(
    client: &Client,
    homeserver_url: &Url,
    token: &str,
    device_id: Option<String>,
    device_display_name: Option<String>,
) -> Result<LoginResponse> {
    let request = LoginRequest {
        login_type: "m.login.token".to_string(),
        user: None,
        password: None,
        device_id,
        initial_device_display_name: device_display_name,
        token: Some(token.to_string()),
        refresh_token: None,
    };

    login(client, homeserver_url, request).await
}

/// Refresh access token using refresh token
pub async fn refresh_access_token(
    client: &Client,
    homeserver_url: &Url,
    refresh_token: &str,
    device_id: Option<String>,
) -> Result<LoginResponse> {
    let request = LoginRequest {
        login_type: "m.login.password".to_string(), // Use password type for refresh
        user: None,
        password: None,
        device_id,
        initial_device_display_name: None,
        token: None,
        refresh_token: Some(refresh_token.to_string()),
    };

    login(client, homeserver_url, request).await
}
