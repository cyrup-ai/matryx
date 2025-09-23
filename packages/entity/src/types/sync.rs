use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct SyncQuery {
    pub filter: Option<String>,
    pub since: Option<String>,
    pub full_state: Option<bool>,
    pub set_presence: Option<String>,
    pub timeout: Option<u64>,
}

#[derive(Serialize)]
pub struct SyncResponse {
    pub next_batch: String,
    pub rooms: RoomsResponse,
    pub presence: PresenceResponse,
    pub account_data: AccountDataResponse,
    pub to_device: ToDeviceResponse,
    pub device_lists: DeviceListsResponse,
    pub device_one_time_keys_count: HashMap<String, u32>,
}

#[derive(Serialize)]
pub struct LiveSyncUpdate {
    pub next_batch: String,
    pub rooms: Option<RoomsUpdate>,
    pub presence: Option<PresenceUpdate>,
    pub account_data: Option<AccountDataUpdate>,
    pub to_device: Option<ToDeviceUpdate>,
    pub device_lists: Option<DeviceListsUpdate>,
}

#[derive(Serialize)]
pub struct RoomsUpdate {
    pub join: Option<HashMap<String, JoinedRoomUpdate>>,
    pub invite: Option<HashMap<String, InvitedRoomUpdate>>,
    pub leave: Option<HashMap<String, LeftRoomUpdate>>,
}

#[derive(Serialize)]
pub struct JoinedRoomUpdate {
    pub timeline: Option<TimelineUpdate>,
    pub state: Option<StateUpdate>,
    pub ephemeral: Option<EphemeralUpdate>,
    pub account_data: Option<AccountDataUpdate>,
    pub unread_notifications: Option<UnreadNotificationsUpdate>,
}

#[derive(Serialize)]
pub struct InvitedRoomUpdate {
    pub invite_state: Option<StateUpdate>,
}

#[derive(Serialize)]
pub struct LeftRoomUpdate {
    pub state: Option<StateUpdate>,
    pub timeline: Option<TimelineUpdate>,
    pub account_data: Option<AccountDataUpdate>,
}

#[derive(Serialize)]
pub struct TimelineUpdate {
    pub events: Vec<Value>,
    pub limited: Option<bool>,
    pub prev_batch: Option<String>,
}

#[derive(Serialize)]
pub struct StateUpdate {
    pub events: Vec<Value>,
}

#[derive(Serialize)]
pub struct EphemeralUpdate {
    pub events: Vec<Value>,
}

#[derive(Serialize)]
pub struct PresenceUpdate {
    pub events: Vec<Value>,
}

#[derive(Serialize)]
pub struct AccountDataUpdate {
    pub events: Vec<Value>,
}

#[derive(Serialize)]
pub struct ToDeviceUpdate {
    pub events: Vec<Value>,
}

#[derive(Serialize)]
pub struct DeviceListsUpdate {
    pub changed: Vec<String>,
    pub left: Vec<String>,
}

#[derive(Serialize)]
pub struct UnreadNotificationsUpdate {
    pub highlight_count: Option<u32>,
    pub notification_count: Option<u32>,
}

#[derive(Serialize)]
pub struct RoomsResponse {
    pub join: HashMap<String, JoinedRoomResponse>,
    pub invite: HashMap<String, InvitedRoomResponse>,
    pub leave: HashMap<String, LeftRoomResponse>,
}

#[derive(Serialize)]
pub struct JoinedRoomResponse {
    pub summary: RoomSummary,
    pub state: StateResponse,
    pub timeline: TimelineResponse,
    pub ephemeral: EphemeralResponse,
    pub account_data: AccountDataResponse,
    pub unread_notifications: UnreadNotifications,
}

#[derive(Serialize)]
pub struct InvitedRoomResponse {
    pub invite_state: StateResponse,
}

#[derive(Serialize)]
pub struct LeftRoomResponse {
    pub state: StateResponse,
    pub timeline: TimelineResponse,
    pub account_data: AccountDataResponse,
}

#[derive(Serialize)]
pub struct RoomSummary {
    #[serde(rename = "m.heroes")]
    pub heroes: Vec<String>,
    #[serde(rename = "m.joined_member_count")]
    pub joined_member_count: u32,
    #[serde(rename = "m.invited_member_count")]
    pub invited_member_count: u32,
}

#[derive(Serialize)]
pub struct StateResponse {
    pub events: Vec<Value>,
}

#[derive(Serialize)]
pub struct TimelineResponse {
    pub events: Vec<Value>,
    pub limited: bool,
    pub prev_batch: Option<String>,
}

#[derive(Serialize)]
pub struct EphemeralResponse {
    pub events: Vec<Value>,
}

#[derive(Serialize)]
pub struct PresenceResponse {
    pub events: Vec<Value>,
}

#[derive(Serialize)]
pub struct AccountDataResponse {
    pub events: Vec<Value>,
}

#[derive(Serialize)]
pub struct ToDeviceResponse {
    pub events: Vec<Value>,
}

#[derive(Serialize)]
pub struct DeviceListsResponse {
    pub changed: Vec<String>,
    pub left: Vec<String>,
}

#[derive(Serialize)]
pub struct UnreadNotifications {
    pub highlight_count: u32,
    pub notification_count: u32,
}
