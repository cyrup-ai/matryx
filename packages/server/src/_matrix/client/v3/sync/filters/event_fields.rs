use matryx_entity::types::Event;

/// Apply event_fields filtering per Matrix specification
/// Based on spec: "dot-separated paths for each property to include"
pub async fn apply_event_fields_filter(
    events: Vec<Event>,
    event_fields: &[String],
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    if event_fields.is_empty() {
        return Ok(events);
    }

    let filtered_events = events
        .into_iter()
        .map(|event| filter_event_fields(event, event_fields))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(filtered_events)
}

/// Filter individual event fields using Matrix dot notation
fn filter_event_fields(
    event: Event,
    field_paths: &[String],
) -> Result<Event, Box<dyn std::error::Error + Send + Sync>> {
    // Convert event to JSON for field filtering
    let event_json = serde_json::to_value(&event)?;

    // Create filtered JSON with only specified fields
    let mut filtered_json = serde_json::Map::new();

    for field_path in field_paths {
        if let Some(value) = extract_json_field(&event_json, field_path) {
            set_json_field(&mut filtered_json, field_path, value);
        }
    }

    // Convert filtered JSON back to Event
    let filtered_event: Event = serde_json::from_value(serde_json::Value::Object(filtered_json))?;

    Ok(filtered_event)
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
