use serde_json::{Value, json};
use std::collections::HashMap;

use crate::state::AppState;
use matryx_entity::filter::EventFilter;
use matryx_entity::types::{AccountData, Event, Membership};
use matryx_surrealdb::repository::{AccountDataRepository, EventRepository, MembershipRepository};

pub async fn get_user_memberships(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Membership>, Box<dyn std::error::Error + Send + Sync>> {
    let membership_repo = MembershipRepository::new(state.db.clone());
    let memberships = membership_repo
        .get_user_rooms(user_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(memberships)
}

pub async fn get_user_account_data(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let account_data_repo = AccountDataRepository::new(state.db.clone());
    let account_data = account_data_repo
        .get_global_for_user(user_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let events = account_data
        .into_iter()
        .map(|data| {
            json!({
                "type": data.account_data_type,
                "content": data.content
            })
        })
        .collect();

    Ok(events)
}

/// Update user presence status
pub async fn set_user_presence(
    state: &AppState,
    user_id: &str,
    presence: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Validate presence value
    let valid_presence = match presence {
        "online" | "offline" | "unavailable" => presence,
        _ => return Err("Invalid presence value".into()),
    };

    // Update or create presence event
    let query = r#"
        UPSERT presence_events:⟨$user_id⟩ CONTENT {
            user_id: $user_id,
            presence: $presence,
            status_msg: NONE,
            last_active_ago: 0,
            currently_active: $currently_active,
            updated_at: time::now()
        }
    "#;

    let currently_active = valid_presence == "online";

    let _: Option<serde_json::Value> = state
        .db
        .query(query)
        .bind(("user_id", user_id.to_string()))
        .bind(("presence", valid_presence.to_string()))
        .bind(("currently_active", currently_active))
        .await?
        .take(0)?;

    Ok(())
}

/// Get room heroes (other prominent members for room summary)
pub async fn get_room_heroes(
    state: &AppState,
    room_id: &str,
    current_user_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Get up to 5 most recently active members excluding current user
    let query = r#"
        SELECT user_id FROM membership
        WHERE room_id = $room_id
        AND membership = 'join'
        AND user_id != $current_user_id
        ORDER BY updated_at DESC
        LIMIT 5
    "#;

    let mut response = state
        .db
        .query(query)
        .bind(("room_id", room_id.to_string()))
        .bind(("current_user_id", current_user_id.to_string()))
        .await?;

    #[derive(serde::Deserialize)]
    struct MemberInfo {
        user_id: String,
    }

    let members: Vec<MemberInfo> = response.take(0)?;
    let heroes = members.into_iter().map(|m| m.user_id).collect();

    Ok(heroes)
}

/// Get count of joined members in a room
pub async fn get_joined_member_count(
    state: &AppState,
    room_id: &str,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let query =
        "SELECT count() FROM membership WHERE room_id = $room_id AND membership = 'join' GROUP ALL";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    let count: Option<i64> = response.take(0)?;
    Ok(count.unwrap_or(0) as u32)
}

/// Get count of invited members in a room
pub async fn get_invited_member_count(
    state: &AppState,
    room_id: &str,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let query = "SELECT count() FROM membership WHERE room_id = $room_id AND membership = 'invite' GROUP ALL";

    let mut response = state.db.query(query).bind(("room_id", room_id.to_string())).await?;

    let count: Option<i64> = response.take(0)?;
    Ok(count.unwrap_or(0) as u32)
}

/// Get presence events for user's contacts and self
pub async fn get_user_presence_events(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let query = r#"
        SELECT * FROM presence_events
        WHERE user_id IN (
            SELECT VALUE target_user_id FROM user_relationships
            WHERE user_id = $user_id AND relationship_type = 'friend'
        )
        OR user_id = $user_id
        ORDER BY updated_at DESC
    "#;

    let user_id_owned = user_id.to_string();
    let mut response = state.db.query(query).bind(("user_id", user_id_owned)).await?;

    #[derive(serde::Deserialize)]
    struct PresenceEvent {
        user_id: String,
        presence: String,
        status_msg: Option<String>,
        last_active_ago: Option<i64>,
        currently_active: bool,
    }

    let presence_events: Vec<PresenceEvent> = response.take(0)?;

    let events: Vec<Value> = presence_events
        .into_iter()
        .map(|event| {
            json!({
                "type": "m.presence",
                "sender": event.user_id,
                "content": {
                    "presence": event.presence,
                    "status_msg": event.status_msg,
                    "last_active_ago": event.last_active_ago,
                    "currently_active": event.currently_active
                }
            })
        })
        .collect();

    Ok(events)
}

/// Get default timeline events without filtering
pub async fn get_default_timeline_events(
    state: &AppState,
    room_id: &str,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = EventRepository::new(state.db.clone());
    let events = event_repo
        .get_room_events(room_id, Some(20))
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(events)
}

/// Get room state events
pub async fn get_room_state_events(
    state: &AppState,
    room_id: &str,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = EventRepository::new(state.db.clone());
    let events = event_repo
        .get_state_events(room_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(events)
}

/// Get room ephemeral events
pub async fn get_room_ephemeral_events(
    state: &AppState,
    room_id: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let events: Vec<Event> = state
        .db
        .query("SELECT * FROM event WHERE room_id = $room_id AND event_type LIKE 'm.typing%' OR event_type LIKE 'm.receipt%'")
        .bind(("room_id", room_id.to_string()))
        .await?
        .take(0)?;

    let json_events = convert_events_to_matrix_format(events);
    Ok(json_events)
}

/// Convert events to Matrix JSON format
pub fn convert_events_to_matrix_format(events: Vec<Event>) -> Vec<Value> {
    events
        .into_iter()
        .map(|event| {
            json!({
                "event_id": event.event_id,
                "sender": event.sender,
                "origin_server_ts": event.origin_server_ts,
                "type": event.event_type,
                "content": event.content,
                "state_key": event.state_key,
                "unsigned": event.unsigned
            })
        })
        .collect()
}

/// Apply presence filtering to sync response
pub async fn apply_presence_filter(
    presence_events: Vec<Value>,
    filter: &EventFilter,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    // Convert Value events to Event objects for filtering
    let events: Vec<Event> = presence_events
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    let filtered =
        crate::_matrix::client::v3::sync::filters::apply_event_filter(events, filter).await?;

    // Convert back to Value format
    let result = filtered.into_iter().filter_map(|e| serde_json::to_value(e).ok()).collect();

    Ok(result)
}

/// Apply account data filtering to sync response
pub async fn apply_account_data_filter(
    account_data: Vec<Value>,
    filter: &EventFilter,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    // Convert Value events to Event objects for filtering
    let events: Vec<Event> = account_data
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    let filtered =
        crate::_matrix::client::v3::sync::filters::apply_event_filter(events, filter).await?;

    // Convert back to Value format
    let result = filtered.into_iter().filter_map(|e| serde_json::to_value(e).ok()).collect();

    Ok(result)
}

/// Get timeline events for a room with optional filtering
pub async fn get_room_timeline_events(
    state: &AppState,
    room_id: &str,
    limit: Option<u32>,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    let event_repo = EventRepository::new(state.db.clone());
    let events = event_repo
        .get_room_events(room_id, limit)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(events)
}

/// Get rooms where user has joined membership
pub async fn get_joined_rooms(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Membership>, Box<dyn std::error::Error + Send + Sync>> {
    let membership_repo = MembershipRepository::new(state.db.clone());
    let memberships = membership_repo
        .get_user_rooms_by_state(user_id, "join")
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(memberships)
}

/// Get rooms where user has invited membership
pub async fn get_invited_rooms(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Membership>, Box<dyn std::error::Error + Send + Sync>> {
    let membership_repo = MembershipRepository::new(state.db.clone());
    let memberships = membership_repo
        .get_user_rooms_by_state(user_id, "invite")
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(memberships)
}

/// Get rooms where user has left membership
pub async fn get_left_rooms(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Membership>, Box<dyn std::error::Error + Send + Sync>> {
    let membership_repo = MembershipRepository::new(state.db.clone());
    let memberships = membership_repo
        .get_user_rooms_by_state(user_id, "leave")
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(memberships)
}
