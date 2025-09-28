use serde_json::Value;


use super::basic_filters::apply_event_filter;
use crate::state::AppState;
use matryx_entity::filter::{EventFilter, RoomEventFilter};
use matryx_entity::types::Event;
use matryx_surrealdb::repository::FilterRepository;

/// Get filtered timeline events with database-level optimizations
pub async fn get_filtered_timeline_events(
    state: &AppState,
    room_id: &str,
    filter: &RoomEventFilter,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    let filter_repo = FilterRepository::new(state.db.clone());
    let events = filter_repo
        .get_filtered_timeline_events(room_id, filter)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    Ok(events)
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

    let filtered = apply_event_filter(events, filter).await?;

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

    let filtered = apply_event_filter(events, filter).await?;

    // Convert back to Value format
    let result = filtered.into_iter().filter_map(|e| serde_json::to_value(e).ok()).collect();

    Ok(result)
}
