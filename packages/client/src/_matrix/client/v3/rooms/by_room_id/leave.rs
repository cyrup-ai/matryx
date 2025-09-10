//! POST /_matrix/client/v3/rooms/{roomId}/leave
//!
//! Leave a room. This endpoint allows users to leave rooms they are currently joined to.
//! Once left, the user will no longer receive events from the room.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use url::Url;

/// Request body for leaving a room
#[derive(Debug, Clone, Serialize)]
pub struct LeaveRoomRequest {
    /// Reason for leaving the room (optional)
    /// This may be displayed in the room timeline to other members
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Response when leaving a room
#[derive(Debug, Clone, Deserialize)]
pub struct LeaveRoomResponse {
    // Matrix spec doesn't specify response fields for leave, typically empty object
}

/// Error response from Matrix server
#[derive(Debug, Clone, Deserialize)]
pub struct MatrixError {
    pub errcode: String,
    pub error: String,
}

/// Result type for client operations
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// POST /_matrix/client/v3/rooms/{roomId}/leave
///
/// Leave a room. The user must be currently joined to the room to leave it.
/// After leaving, the user will no longer receive events from the room.
pub async fn leave_room(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id: &str,
    request: Option<LeaveRoomRequest>,
) -> Result<LeaveRoomResponse> {
    let url = homeserver_url
        .join(&format!("/_matrix/client/v3/rooms/{}/leave", urlencoding::encode(room_id)))?;

    debug!("Leaving room: {}", room_id);

    let request_body = request.unwrap_or(LeaveRoomRequest { reason: None });

    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("User-Agent", "Matryx-Client/0.1.0")
        .json(&request_body)
        .send()
        .await?;

    if response.status().is_success() {
        let leave_response: LeaveRoomResponse =
            response.json().await.unwrap_or(LeaveRoomResponse {});
        info!("Successfully left room: {}", room_id);
        Ok(leave_response)
    } else {
        let status = response.status();
        let error_text = response.text().await?;

        // Try to parse as Matrix error
        if let Ok(matrix_error) = serde_json::from_str::<MatrixError>(&error_text) {
            error!(
                "Matrix server error leaving room: {} - {}",
                matrix_error.errcode, matrix_error.error
            );
            return Err(
                format!("Matrix error {}: {}", matrix_error.errcode, matrix_error.error).into()
            );
        }

        error!("HTTP error leaving room: {} - {}", status, error_text);
        Err(format!("HTTP error {}: {}", status, error_text).into())
    }
}

/// Leave a room without providing a reason
///
/// Simple convenience function for leaving a room.
pub async fn leave_room_simple(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id: &str,
) -> Result<LeaveRoomResponse> {
    leave_room(client, homeserver_url, access_token, room_id, None).await
}

/// Leave a room with a reason
///
/// The reason may be displayed in the room timeline to inform other members
/// why the user left the room.
pub async fn leave_room_with_reason(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id: &str,
    reason: &str,
) -> Result<LeaveRoomResponse> {
    let request = LeaveRoomRequest { reason: Some(reason.to_string()) };

    leave_room(client, homeserver_url, access_token, room_id, Some(request)).await
}
