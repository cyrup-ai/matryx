use serde_json::{Value, json};


use crate::state::AppState;
use matryx_entity::filter::EventFilter;
use matryx_entity::types::{Event, Membership};
use matryx_surrealdb::repository::{AccountDataRepository, EventRepository, MembershipRepository};



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
    use matryx_surrealdb::repository::{PresenceRepository, PresenceState};
    
    // Validate presence value and convert to PresenceState
    let presence_state = match presence {
        "online" => PresenceState::Online,
        "offline" => PresenceState::Offline,
        "unavailable" => PresenceState::Unavailable,
        _ => return Err("Invalid presence value".into()),
    };

    let presence_repo = PresenceRepository::new(state.db.clone());
    presence_repo.update_user_presence_state(user_id, presence_state, None).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(())
}

/// Get room heroes (other prominent members for room summary)
pub async fn get_room_heroes(
    state: &AppState,
    room_id: &str,
    current_user_id: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    use matryx_surrealdb::repository::SyncRepository;
    
    let sync_repo = SyncRepository::new(state.db.clone());
    let heroes = sync_repo
        .get_room_heroes(room_id, current_user_id)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(heroes)
}

/// Get count of joined members in a room
pub async fn get_joined_member_count(
    state: &AppState,
    room_id: &str,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    use matryx_surrealdb::repository::SyncRepository;
    
    let sync_repo = SyncRepository::new(state.db.clone());
    let count = sync_repo.get_room_member_count(room_id).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(count)
}

/// Get count of invited members in a room
pub async fn get_invited_member_count(
    state: &AppState,
    room_id: &str,
) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    use matryx_surrealdb::repository::SyncRepository;
    
    let sync_repo = SyncRepository::new(state.db.clone());
    let count = sync_repo.get_room_invited_member_count(room_id).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    Ok(count)
}

/// Get presence events for user's contacts and self
pub async fn get_user_presence_events(
    state: &AppState,
    user_id: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    use matryx_surrealdb::repository::PresenceRepository;
    
    let presence_repo = PresenceRepository::new(state.db.clone());
    let presence_events = presence_repo.get_user_presence_events(user_id, None).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

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
    use matryx_surrealdb::repository::SyncRepository;
    
    let sync_repo = SyncRepository::new(state.db.clone());
    let ephemeral_events = sync_repo.get_room_ephemeral_events(room_id, None).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let json_events: Vec<Value> = ephemeral_events
        .into_iter()
        .map(|event| {
            json!({
                "event_id": event.event_id,
                "sender": event.sender,
                "type": event.event_type,
                "content": event.content
            })
        })
        .collect();

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
