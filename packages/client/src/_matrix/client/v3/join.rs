//! POST /_matrix/client/v3/join/{roomIdOrAlias}
//!
//! Join a room by room ID or room alias. This is one of the core Matrix operations
//! that allows users to participate in rooms.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, info};
use url::Url;

/// Request body for joining a room
#[derive(Debug, Clone, Serialize)]
pub struct JoinRoomRequest {
    /// Third-party signed data for joining restricted rooms
    #[serde(skip_serializing_if = "Option::is_none")]
    pub third_party_signed: Option<ThirdPartySigned>,

    /// Reason for joining the room (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Third-party signed data for joining restricted rooms
#[derive(Debug, Clone, Serialize)]
pub struct ThirdPartySigned {
    pub sender: String,
    pub mxid: String,
    pub token: String,
    pub signatures: Value,
}

/// Response when joining a room
#[derive(Debug, Clone, Deserialize)]
pub struct JoinRoomResponse {
    /// The room ID that was joined
    pub room_id: String,
}

/// Error response from Matrix server
#[derive(Debug, Clone, Deserialize)]
pub struct MatrixError {
    pub errcode: String,
    pub error: String,
}

/// Result type for client operations
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// POST /_matrix/client/v3/join/{roomIdOrAlias}
///
/// Join a room. The room identifier can be either:
/// - A room ID (e.g., "!example:matrix.org")
/// - A room alias (e.g., "#general:matrix.org")
pub async fn join_room(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id_or_alias: &str,
    request: Option<JoinRoomRequest>,
) -> Result<JoinRoomResponse> {
    let url = homeserver_url
        .join(&format!("/_matrix/client/v3/join/{}", urlencoding::encode(room_id_or_alias)))?;

    debug!("Joining room: {}", room_id_or_alias);

    let request_body =
        request.unwrap_or(JoinRoomRequest { third_party_signed: None, reason: None });

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("User-Agent", "Matryx-Client/0.1.0")
        .json(&request_body)
        .send()
        .await?;

    if response.status().is_success() {
        let join_response: JoinRoomResponse = response.json().await?;
        info!(
            "Successfully joined room {} (actual room ID: {})",
            room_id_or_alias, join_response.room_id
        );
        Ok(join_response)
    } else {
        let status = response.status();
        let error_text = response.text().await?;

        // Try to parse as Matrix error
        if let Ok(matrix_error) = serde_json::from_str::<MatrixError>(&error_text) {
            error!(
                "Matrix server error joining room: {} - {}",
                matrix_error.errcode, matrix_error.error
            );
            return Err(
                format!("Matrix error {}: {}", matrix_error.errcode, matrix_error.error).into()
            );
        }

        error!("HTTP error joining room: {} - {}", status, error_text);
        Err(format!("HTTP error {}: {}", status, error_text).into())
    }
}

/// Join a room by room ID
///
/// Convenience function for joining by room ID (e.g., "!example:matrix.org").
pub async fn join_room_by_id(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id: &str,
) -> Result<JoinRoomResponse> {
    join_room(client, homeserver_url, access_token, room_id, None).await
}

/// Join a room by room alias
///
/// Convenience function for joining by room alias (e.g., "#general:matrix.org").
pub async fn join_room_by_alias(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_alias: &str,
) -> Result<JoinRoomResponse> {
    join_room(client, homeserver_url, access_token, room_alias, None).await
}

/// Join a room with a reason
///
/// Some rooms may display the reason for joining in the room timeline.
pub async fn join_room_with_reason(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id_or_alias: &str,
    reason: &str,
) -> Result<JoinRoomResponse> {
    let request = JoinRoomRequest {
        third_party_signed: None,
        reason: Some(reason.to_string()),
    };

    join_room(client, homeserver_url, access_token, room_id_or_alias, Some(request)).await
}

/// Join a restricted room with third-party signed data
///
/// Some rooms require third-party authorization to join.
pub async fn join_restricted_room(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id_or_alias: &str,
    third_party_signed: ThirdPartySigned,
    reason: Option<String>,
) -> Result<JoinRoomResponse> {
    let request = JoinRoomRequest {
        third_party_signed: Some(third_party_signed),
        reason,
    };

    join_room(client, homeserver_url, access_token, room_id_or_alias, Some(request)).await
}
