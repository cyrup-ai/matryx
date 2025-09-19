use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, info};
use url::Url;

/// Room creation request
#[derive(Debug, Clone, Serialize)]
pub struct CreateRoomRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_alias_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite_3pid: Option<Vec<Invite3pid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_content: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_state: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_direct: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_level_content_override: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Invite3pid {
    pub id_server: String,
    pub id_access_token: String,
    pub medium: String,
    pub address: String,
}
/// Room creation response
#[derive(Debug, Clone, Deserialize)]
pub struct CreateRoomResponse {
    pub room_id: String,
}

/// POST /_matrix/client/v3/createRoom - Create a new room
pub async fn create_room(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    request: CreateRoomRequest,
) -> Result<CreateRoomResponse> {
    let url = homeserver_url.join("/_matrix/client/v3/createRoom")?;

    debug!("Creating room with request: {:?}", request);

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", access_token))
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

            error!("Room creation failed with Matrix error {}: {}", errcode, error_msg);
            return Err(anyhow::anyhow!("Room creation failed: {} ({})", error_msg, errcode));
        }
        error!("Room creation request failed: {} - {}", status, error_body);
        return Err(anyhow::anyhow!("Room creation request failed: {} - {}", status, error_body));
    }

    let create_response = response.json::<CreateRoomResponse>().await?;

    info!("Successfully created room: {}", create_response.room_id);
    Ok(create_response)
}

/// Create a simple private room with name and topic
pub async fn create_simple_room(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    name: Option<String>,
    topic: Option<String>,
    is_private: bool,
) -> Result<CreateRoomResponse> {
    let request = CreateRoomRequest {
        visibility: Some(if is_private {
            "private".to_string()
        } else {
            "public".to_string()
        }),
        room_alias_name: None,
        name,
        topic,
        invite: None,
        invite_3pid: None,
        room_version: None,
        creation_content: None,
        initial_state: None,
        preset: Some(if is_private {
            "private_chat".to_string()
        } else {
            "public_chat".to_string()
        }),
        is_direct: Some(false),
        power_level_content_override: None,
    };

    create_room(client, homeserver_url, access_token, request).await
}

/// Create a direct message room with another user
pub async fn create_direct_room(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    invite_user_id: &str,
) -> Result<CreateRoomResponse> {
    let request = CreateRoomRequest {
        visibility: Some("private".to_string()),
        room_alias_name: None,
        name: None,
        topic: None,
        invite: Some(vec![invite_user_id.to_string()]),
        invite_3pid: None,
        room_version: None,
        creation_content: None,
        initial_state: None,
        preset: Some("private_chat".to_string()),
        is_direct: Some(true),
        power_level_content_override: None,
    };

    create_room(client, homeserver_url, access_token, request).await
}
