use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, info};
use url::Url;

/// Sync request parameters
#[derive(Debug, Clone, Serialize)]
pub struct SyncRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_state: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_presence: Option<String>,
}

/// Complete sync response from Matrix server
#[derive(Debug, Clone, Deserialize)]
pub struct SyncResponse {
    pub next_batch: String,
    pub rooms: RoomUpdates,
    #[serde(default)]
    pub presence: PresenceUpdates,
    #[serde(default)]
    pub account_data: AccountDataUpdates,
    #[serde(default)]
    pub to_device: ToDeviceUpdates,
    #[serde(default)]
    pub device_lists: DeviceListUpdates,
    #[serde(default)]
    pub device_one_time_keys_count: HashMap<String, u64>,
    #[serde(default)]
    pub device_unused_fallback_key_types: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RoomUpdates {
    #[serde(default)]
    pub join: HashMap<String, JoinedRoomUpdate>,
    #[serde(default)]
    pub invite: HashMap<String, InvitedRoomUpdate>,
    #[serde(default)]
    pub leave: HashMap<String, LeftRoomUpdate>,
    #[serde(default)]
    pub knock: HashMap<String, KnockedRoomUpdate>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct JoinedRoomUpdate {
    #[serde(default)]
    pub state: StateUpdate,
    #[serde(default)]
    pub timeline: TimelineUpdate,
    #[serde(default)]
    pub ephemeral: EphemeralUpdate,
    #[serde(default)]
    pub account_data: AccountDataUpdate,
    #[serde(default)]
    pub unread_notifications: UnreadNotifications,
    #[serde(default)]
    pub summary: Option<RoomSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InvitedRoomUpdate {
    pub invite_state: StateUpdate,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LeftRoomUpdate {
    #[serde(default)]
    pub state: StateUpdate,
    #[serde(default)]
    pub timeline: TimelineUpdate,
    #[serde(default)]
    pub account_data: AccountDataUpdate,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KnockedRoomUpdate {
    pub knock_state: StateUpdate,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct StateUpdate {
    #[serde(default)]
    pub events: Vec<Value>,
}
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TimelineUpdate {
    #[serde(default)]
    pub events: Vec<Value>,
    #[serde(default)]
    pub limited: bool,
    pub prev_batch: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EphemeralUpdate {
    #[serde(default)]
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AccountDataUpdate {
    #[serde(default)]
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UnreadNotifications {
    #[serde(default)]
    pub highlight_count: u64,
    #[serde(default)]
    pub notification_count: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoomSummary {
    #[serde(rename = "m.heroes")]
    pub heroes: Option<Vec<String>>,
    #[serde(rename = "m.joined_member_count")]
    pub joined_member_count: Option<u64>,
    #[serde(rename = "m.invited_member_count")]
    pub invited_member_count: Option<u64>,
}
#[derive(Debug, Clone, Deserialize, Default)]
pub struct PresenceUpdates {
    #[serde(default)]
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AccountDataUpdates {
    #[serde(default)]
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ToDeviceUpdates {
    #[serde(default)]
    pub events: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DeviceListUpdates {
    #[serde(default)]
    pub changed: Vec<String>,
    #[serde(default)]
    pub left: Vec<String>,
}

/// GET /_matrix/client/v3/sync - Synchronize with Matrix server
pub async fn sync(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    request: SyncRequest,
) -> Result<SyncResponse> {
    let mut url = homeserver_url.join("/_matrix/client/v3/sync")?;

    // Build query parameters
    let mut query_params = Vec::new();

    // Create timeout string with appropriate lifetime
    let timeout_str = request.timeout.map(|t| t.to_string());

    if let Some(ref since) = request.since {
        query_params.push(("since", since.as_str()));
    }

    if let Some(ref timeout_s) = timeout_str {
        query_params.push(("timeout", timeout_s.as_str()));
    }

    if let Some(ref filter) = request.filter {
        query_params.push(("filter", filter.as_str()));
    }

    if let Some(full_state) = request.full_state {
        query_params.push(("full_state", if full_state { "true" } else { "false" }));
    }

    if let Some(ref presence) = request.set_presence {
        query_params.push(("set_presence", presence.as_str()));
    }

    // Set query string if we have parameters
    if !query_params.is_empty() {
        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        url.set_query(Some(&query_string));
    }

    debug!("Sync request to: {}", url);

    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "Matryx-Client/0.1.0")
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

            error!("Sync failed with Matrix error {}: {}", errcode, error_msg);
            return Err(anyhow::anyhow!("Sync failed: {} ({})", error_msg, errcode));
        }

        error!("Sync request failed: {} - {}", status, error_body);
        return Err(anyhow::anyhow!("Sync request failed: {} - {}", status, error_body));
    }

    let sync_response = response.json::<SyncResponse>().await?;

    debug!("Sync successful, next_batch: {}", sync_response.next_batch);
    info!(
        "Sync: {} joined, {} invited, {} left rooms",
        sync_response.rooms.join.len(),
        sync_response.rooms.invite.len(),
        sync_response.rooms.leave.len()
    );

    Ok(sync_response)
}

/// Perform initial sync
pub async fn initial_sync(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    timeout: Option<u64>,
) -> Result<SyncResponse> {
    let request = SyncRequest {
        since: None,
        timeout,
        filter: None,
        full_state: Some(true),
        set_presence: Some("online".to_string()),
    };

    sync(client, homeserver_url, access_token, request).await
}

/// Continue sync from previous batch token
pub async fn continue_sync(
    client: &Client,
    homeserver_url: &Url,
    access_token: &str,
    since: &str,
    timeout: Option<u64>,
) -> Result<SyncResponse> {
    let request = SyncRequest {
        since: Some(since.to_string()),
        timeout,
        filter: None,
        full_state: Some(false),
        set_presence: None,
    };

    sync(client, homeserver_url, access_token, request).await
}
