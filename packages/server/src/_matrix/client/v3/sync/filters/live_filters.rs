use futures::stream::{Stream, StreamExt};
use std::pin::Pin;

use super::database_filters::{apply_account_data_filter, apply_presence_filter};
use crate::state::AppState;
use matryx_entity::types::{LiveSyncUpdate, MatrixFilter};

/// Apply filter to a sync update
pub async fn apply_filter_to_update(
    update: LiveSyncUpdate,
    filter: &MatrixFilter,
) -> Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>> {
    // Apply the Matrix filter to the sync update
    match apply_matrix_filter_to_update(update, filter).await? {
        Some(filtered_update) => Ok(filtered_update),
        None => {
            // Update was completely filtered out - return error to indicate no data
            Err("Update filtered out completely".into())
        },
    }
}

/// Internal function to create a live filtered sync stream
pub async fn create_live_filtered_stream(
    state: AppState,
    user_id: String,
    filter: MatrixFilter,
) -> Result<
    Pin<
        Box<
            dyn Stream<Item = Result<LiveSyncUpdate, Box<dyn std::error::Error + Send + Sync>>>
                + Send,
        >,
    >,
    Box<dyn std::error::Error + Send + Sync>,
> {
    // Import the existing event stream creation function
    use super::super::streaming::event_streams::create_event_live_stream;

    // Create the base event stream
    let base_stream = create_event_live_stream(state, user_id).await?;

    // Apply the filter to each update in the stream
    let filtered_stream = base_stream.filter_map(move |update_result| {
        let filter = filter.clone();
        async move {
            match update_result {
                Ok(update) => {
                    // Apply the Matrix filter to the sync update
                    match apply_matrix_filter_to_update(update, &filter).await {
                        Ok(Some(filtered_update)) => Some(Ok(filtered_update)),
                        Ok(None) => None, // Update filtered out
                        Err(e) => Some(Err(e)),
                    }
                },
                Err(e) => Some(Err(e)), // Pass through errors
            }
        }
    });

    Ok(Box::pin(filtered_stream))
}

/// Apply Matrix filter specification to a sync update
async fn apply_matrix_filter_to_update(
    update: LiveSyncUpdate,
    filter: &MatrixFilter,
) -> Result<Option<LiveSyncUpdate>, Box<dyn std::error::Error + Send + Sync>> {
    // Validate event_format per Matrix spec (line 787)
    if filter.event_format != "client" && filter.event_format != "federation" {
        return Err(format!("Invalid event_format: '{}'. Must be 'client' or 'federation'", filter.event_format).into());
    }
    if filter.event_format == "federation" {
        tracing::warn!("Federation event_format requested - transformation not yet implemented (Part B)");
    }

    let mut filtered_update = update;

    // Apply event_fields filtering per Matrix spec (line 786)
    if let Some(ref event_fields) = filter.event_fields
        && !event_fields.is_empty()
    {
        // Apply event_fields to presence events
        if let Some(ref mut presence_update) = filtered_update.presence {
            presence_update.events = apply_event_fields_to_json_events(&presence_update.events, event_fields)?;
        }

        // Apply event_fields to account_data events
        if let Some(ref mut account_data_update) = filtered_update.account_data {
            account_data_update.events = apply_event_fields_to_json_events(&account_data_update.events, event_fields)?;
        }

        // Room event filtering will be handled in Part B
    }

    // Apply presence filter per Matrix spec
    if let Some(presence_filter) = &filter.presence
        && let Some(ref mut presence_update) = filtered_update.presence
    {
        // The events are already in Value format, so we can apply the filter directly
        let presence_events = std::mem::take(&mut presence_update.events);

        // Apply the presence filter
        match apply_presence_filter(presence_events, presence_filter).await {
            Ok(filtered_events) => {
                // Apply filtered results to the update
                presence_update.events = filtered_events;
                tracing::debug!(
                    "Applied presence filter, {} events remain",
                    presence_update.events.len()
                );
            },
            Err(e) => {
                tracing::error!("Failed to apply presence filter: {}", e);
            },
        }
    }

    // Apply account data filter per Matrix spec
    if let Some(account_data_filter) = &filter.account_data
        && let Some(ref mut account_data_update) = filtered_update.account_data
    {
        // The events are already in Value format, so we can apply the filter directly
        let account_data_events = std::mem::take(&mut account_data_update.events);

        // Apply the account data filter
        match apply_account_data_filter(account_data_events, account_data_filter).await {
            Ok(filtered_events) => {
                // Apply filtered results to the update
                account_data_update.events = filtered_events;
                tracing::debug!(
                    "Applied account data filter, {} events remain",
                    account_data_update.events.len()
                );
            },
            Err(e) => {
                tracing::error!("Failed to apply account data filter: {}", e);
            },
        }
    }

    // Apply room filter
    if let Some(room_filter) = &filter.room
        && let Some(ref mut rooms_update) = filtered_update.rooms
    {
        // Apply filters to joined rooms
        if let Some(ref mut join_rooms) = rooms_update.join {
                for (_room_id, room_update) in join_rooms.iter_mut() {
                    // Apply timeline filtering
                    if let Some(timeline_filter) = &room_filter.timeline
                        && let Some(ref mut timeline) = room_update.timeline
                    {
                            use super::basic_filters::apply_event_filter;
                            use matryx_entity::types::Event;
                            let events: Vec<Event> = timeline.events.iter()
                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                .collect();
                            
                            match apply_event_filter(events, &timeline_filter.base).await {
                                Ok(filtered_events) => {
                                    timeline.events = filtered_events.into_iter()
                                        .filter_map(|e| serde_json::to_value(e).ok())
                                        .collect();
                                },
                                Err(e) => {
                                    tracing::error!("Failed to apply timeline filter: {}", e);
                                }
                            }
                    }
                    
                    // Apply state filtering
                    if let Some(state_filter) = &room_filter.state
                        && let Some(ref mut state) = room_update.state
                    {
                            use super::basic_filters::apply_event_filter;
                            use matryx_entity::types::Event;
                            let events: Vec<Event> = state.events.iter()
                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                .collect();
                            
                            match apply_event_filter(events, &state_filter.base).await {
                                Ok(filtered_events) => {
                                    state.events = filtered_events.into_iter()
                                        .filter_map(|e| serde_json::to_value(e).ok())
                                        .collect();
                                },
                                Err(e) => {
                                    tracing::error!("Failed to apply state filter: {}", e);
                                }
                            }
                    }
                    
                    // Apply ephemeral filtering
                    if let Some(ephemeral_filter) = &room_filter.ephemeral
                        && let Some(ref mut ephemeral) = room_update.ephemeral
                    {
                            use super::basic_filters::apply_event_filter;
                            use matryx_entity::types::Event;
                            let events: Vec<Event> = ephemeral.events.iter()
                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                .collect();
                            
                            match apply_event_filter(events, &ephemeral_filter.base).await {
                                Ok(filtered_events) => {
                                    ephemeral.events = filtered_events.into_iter()
                                        .filter_map(|e| serde_json::to_value(e).ok())
                                        .collect();
                                },
                                Err(e) => {
                                    tracing::error!("Failed to apply ephemeral filter: {}", e);
                                }
                            }
                    }
                }
        }
    }

    // Return the filtered update
    Ok(Some(filtered_update))
}

/// Apply event_fields filtering to JSON events
fn apply_event_fields_to_json_events(
    events: &[serde_json::Value],
    event_fields: &[String],
) -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>> {
    let filtered_events = events
        .iter()
        .map(|event| filter_json_event_fields(event, event_fields))
        .collect::<Result<Vec<_>, _>>()?;
    
    Ok(filtered_events)
}

/// Filter individual JSON event fields using Matrix dot notation
fn filter_json_event_fields(
    event: &serde_json::Value,
    field_paths: &[String],
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let mut filtered_json = serde_json::Map::new();

    for field_path in field_paths {
        if let Some(value) = extract_json_field(event, field_path) {
            set_json_field(&mut filtered_json, field_path, value);
        }
    }

    Ok(serde_json::Value::Object(filtered_json))
}

/// Extract field from JSON using Matrix dot-separated path notation
fn extract_json_field(json: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json;

    for part in parts {
        current = current.get(part)?;
    }

    Some(current.clone())
}

/// Set field in JSON using Matrix dot-separated path notation
fn set_json_field(
    json: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
    value: serde_json::Value,
) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        json.insert(parts[0].to_string(), value);
        return;
    }

    // Navigate to the parent object
    let mut current_map = json;
    for part in &parts[..parts.len() - 1] {
        let entry = current_map
            .entry(part.to_string())
            .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

        match entry {
            serde_json::Value::Object(map) => {
                current_map = map;
            },
            _ => return, // Invalid path structure
        }
    }

    // Insert the final value
    if let Some(final_key) = parts.last() {
        current_map.insert(final_key.to_string(), value);
    }
}
