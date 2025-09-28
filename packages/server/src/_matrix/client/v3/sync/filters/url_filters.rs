use matryx_entity::types::Event;

/// Implement Matrix-compliant URL detection in event content
pub async fn apply_contains_url_filter(
    events: Vec<Event>,
    contains_url: bool,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    let filtered = events
        .into_iter()
        .filter(|event| {
            let has_url = detect_urls_in_event(event);
            has_url == contains_url
        })
        .collect();

    Ok(filtered)
}

/// Detect URLs in event content across different event types
pub fn detect_urls_in_event(event: &Event) -> bool {
    // Check for URLs in various event content fields
    if let Ok(content_value) = serde_json::to_value(&event.content) {
        detect_urls_in_json(&content_value)
    } else {
        false
    }
}

/// Recursively detect URLs in JSON content
pub fn detect_urls_in_json(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(s) => {
            s.contains("http://") || s.contains("https://") || s.contains("mxc://")
        },
        serde_json::Value::Object(map) => {
            map.get("url").is_some() || map.values().any(detect_urls_in_json)
        },
        serde_json::Value::Array(arr) => arr.iter().any(detect_urls_in_json),
        _ => false,
    }
}
