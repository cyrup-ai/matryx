use matryx_entity::types::Event;
use serde_json::{Value, json};

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
