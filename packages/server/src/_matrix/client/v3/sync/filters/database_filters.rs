use serde_json::Value;
use std::collections::HashMap;

use super::basic_filters::apply_event_filter;
use crate::state::AppState;
use matryx_entity::filter::{EventFilter, RoomEventFilter};
use matryx_entity::types::Event;

/// Get filtered timeline events with database-level optimizations
pub async fn get_filtered_timeline_events(
    state: &AppState,
    room_id: &str,
    filter: &RoomEventFilter,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    let limit = filter.base.limit.unwrap_or(20) as i32;
    let mut query = "SELECT * FROM event WHERE room_id = $room_id".to_string();
    let mut bindings = vec![("room_id", room_id.to_string())];

    // Add event type filtering at database level for performance
    if let Some(types) = &filter.base.types {
        if !types.is_empty() && !types.contains(&"*".to_string()) {
            let type_conditions: Vec<String> = types
                .iter()
                .map(|t| {
                    if t.ends_with('*') {
                        format!("event_type LIKE '{}'", t.replace('*', "%"))
                    } else {
                        format!("event_type = '{}'", t)
                    }
                })
                .collect();
            query.push_str(&format!(" AND ({})", type_conditions.join(" OR ")));
        }
    }

    // Add sender filtering at database level
    if let Some(senders) = &filter.base.senders {
        if !senders.is_empty() {
            let sender_list =
                senders.iter().map(|s| format!("'{}'", s)).collect::<Vec<_>>().join(",");
            query.push_str(&format!(" AND sender IN ({})", sender_list));
        }
    }

    // Add not_senders filtering at database level
    if let Some(not_senders) = &filter.base.not_senders {
        if !not_senders.is_empty() {
            let not_sender_list = not_senders
                .iter()
                .map(|s| format!("'{}'", s))
                .collect::<Vec<_>>()
                .join(",");
            query.push_str(&format!(" AND sender NOT IN ({})", not_sender_list));
        }
    }

    query.push_str(" ORDER BY origin_server_ts DESC LIMIT $limit");
    bindings.push(("limit", limit.to_string()));

    let events: Vec<Event> = state
        .db
        .query(&query)
        .bind(bindings.into_iter().collect::<std::collections::HashMap<_, _>>())
        .await?
        .take(0)?;

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
