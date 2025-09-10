//! PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}
//!
//! Send a message event to a room. This is the primary way to send messages in Matrix.
//! Each message must have a unique transaction ID to prevent duplicates.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, info};
use url::Url;

/// Standard message event types
pub mod event_types {
    pub const TEXT_MESSAGE: &str = "m.room.message";
    pub const MEMBER: &str = "m.room.member";
    pub const TOPIC: &str = "m.room.topic";
    pub const NAME: &str = "m.room.name";
    pub const AVATAR: &str = "m.room.avatar";
}

/// Message content for m.room.message events
#[derive(Debug, Clone, Serialize)]
pub struct MessageContent {
    /// The textual representation of this message
    pub body: String,

    /// The MIME type of the message, e.g. "text/plain" or "text/html"
    #[serde(rename = "msgtype")]
    pub msg_type: String,

    /// Optional formatted body (HTML)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub formatted_body: Option<String>,

    /// The format used in the formatted_body (usually "org.matrix.custom.html")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    /// Additional fields for specific message types
    #[serde(flatten)]
    pub extra: Value,
}

impl MessageContent {
    /// Create a simple text message
    pub fn text<S: Into<String>>(body: S) -> Self {
        Self {
            body: body.into(),
            msg_type: "m.text".to_string(),
            formatted_body: None,
            format: None,
            extra: Value::Object(serde_json::Map::new()),
        }
    }

    /// Create an HTML message with both plain and formatted content
    pub fn html<S: Into<String>>(body: S, html_body: S) -> Self {
        Self {
            body: body.into(),
            msg_type: "m.text".to_string(),
            formatted_body: Some(html_body.into()),
            format: Some("org.matrix.custom.html".to_string()),
            extra: Value::Object(serde_json::Map::new()),
        }
    }

    /// Create an emote message (like /me)
    pub fn emote<S: Into<String>>(body: S) -> Self {
        Self {
            body: body.into(),
            msg_type: "m.emote".to_string(),
            formatted_body: None,
            format: None,
            extra: Value::Object(serde_json::Map::new()),
        }
    }

    /// Create a notice message (automated/bot messages)
    pub fn notice<S: Into<String>>(body: S) -> Self {
        Self {
            body: body.into(),
            msg_type: "m.notice".to_string(),
            formatted_body: None,
            format: None,
            extra: Value::Object(serde_json::Map::new()),
        }
    }
}

/// Response when sending a message event
#[derive(Debug, Clone, Deserialize)]
pub struct SendEventResponse {
    /// The event ID of the sent event
    pub event_id: String,
}

/// Error response from Matrix server
#[derive(Debug, Clone, Deserialize)]
pub struct MatrixError {
    pub errcode: String,
    pub error: String,
}

/// Result type for client operations
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}
///
/// Send an event to a room. This endpoint allows sending any event type.
/// For messages, use the helper functions or send_message for convenience.
pub async fn send_event(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id: &str,
    event_type: &str,
    txn_id: &str,
    content: &Value,
) -> Result<SendEventResponse> {
    let url = homeserver_url.join(&format!(
        "/_matrix/client/v3/rooms/{}/send/{}/{}",
        urlencoding::encode(room_id),
        urlencoding::encode(event_type),
        urlencoding::encode(txn_id)
    ))?;

    debug!("Sending {} event to room {} with txn_id {}", event_type, room_id, txn_id);

    let response = client
        .put(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .header("User-Agent", "Matryx-Client/0.1.0")
        .json(content)
        .send()
        .await?;

    if response.status().is_success() {
        let send_response: SendEventResponse = response.json().await?;
        info!(
            "Successfully sent {} event to room {} with event_id {}",
            event_type, room_id, send_response.event_id
        );
        Ok(send_response)
    } else {
        let status = response.status();
        let error_text = response.text().await?;

        // Try to parse as Matrix error
        if let Ok(matrix_error) = serde_json::from_str::<MatrixError>(&error_text) {
            error!(
                "Matrix server error sending event: {} - {}",
                matrix_error.errcode, matrix_error.error
            );
            return Err(
                format!("Matrix error {}: {}", matrix_error.errcode, matrix_error.error).into()
            );
        }

        error!("HTTP error sending event: {} - {}", status, error_text);
        Err(format!("HTTP error {}: {}", status, error_text).into())
    }
}

/// Send a message to a room (convenience function for m.room.message events)
///
/// This is the most commonly used function for sending text messages.
pub async fn send_message(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id: &str,
    txn_id: &str,
    message: &MessageContent,
) -> Result<SendEventResponse> {
    let content = serde_json::to_value(message)?;
    send_event(
        client,
        homeserver_url,
        access_token,
        room_id,
        event_types::TEXT_MESSAGE,
        txn_id,
        &content,
    )
    .await
}

/// Send a simple text message to a room
///
/// This is a convenience function that creates the message content and transaction ID automatically.
pub async fn send_text_message(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id: &str,
    body: &str,
) -> Result<SendEventResponse> {
    let message = MessageContent::text(body);
    let txn_id = generate_transaction_id();

    send_message(client, homeserver_url, access_token, room_id, &txn_id, &message).await
}

/// Send an HTML message to a room
///
/// This sends a message with both plain text and HTML formatted versions.
pub async fn send_html_message(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id: &str,
    plain_body: &str,
    html_body: &str,
) -> Result<SendEventResponse> {
    let message = MessageContent::html(plain_body, html_body);
    let txn_id = generate_transaction_id();

    send_message(client, homeserver_url, access_token, room_id, &txn_id, &message).await
}

/// Send an emote message (like /me action)
pub async fn send_emote(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id: &str,
    body: &str,
) -> Result<SendEventResponse> {
    let message = MessageContent::emote(body);
    let txn_id = generate_transaction_id();

    send_message(client, homeserver_url, access_token, room_id, &txn_id, &message).await
}

/// Send a notice message (for bots/automated systems)
pub async fn send_notice(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id: &str,
    body: &str,
) -> Result<SendEventResponse> {
    let message = MessageContent::notice(body);
    let txn_id = generate_transaction_id();

    send_message(client, homeserver_url, access_token, room_id, &txn_id, &message).await
}

/// Generate a unique transaction ID for this message
///
/// Transaction IDs prevent duplicate messages if the same request is sent multiple times.
/// They should be unique per client session.
fn generate_transaction_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let counter = COUNTER.fetch_add(1, Ordering::SeqCst);

    format!("m{}.{}", timestamp, counter)
}

/// Check if a message was successfully sent by verifying the event exists
///
/// This can be used to verify message delivery by checking if the event
/// appears in the room's event stream.
pub async fn verify_message_sent(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    room_id: &str,
    event_id: &str,
) -> Result<bool> {
    let url = homeserver_url.join(&format!(
        "/_matrix/client/v3/rooms/{}/event/{}",
        urlencoding::encode(room_id),
        urlencoding::encode(event_id)
    ))?;

    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "Matryx-Client/0.1.0")
        .send()
        .await?;

    Ok(response.status().is_success())
}
