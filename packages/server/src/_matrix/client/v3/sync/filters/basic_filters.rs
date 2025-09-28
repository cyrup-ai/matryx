use axum::http::StatusCode;

use crate::state::AppState;
use matryx_entity::filter::EventFilter;
use matryx_entity::types::{Event, MatrixFilter, Membership, MembershipState};
use matryx_surrealdb::repository::FilterRepository;

/// Resolve a filter parameter to a MatrixFilter object
pub async fn resolve_filter(
    state: &AppState,
    filter_param: &str,
    _user_id: &str,
) -> Result<Option<MatrixFilter>, StatusCode> {
    if filter_param.starts_with('{') {
        // Inline filter definition
        serde_json::from_str(filter_param)
            .map(Some)
            .map_err(|_| StatusCode::BAD_REQUEST)
    } else {
        // Filter ID reference - use existing repository
        let filter_repo = FilterRepository::new(state.db.clone());
        filter_repo
            .get_by_id(filter_param)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }
}

/// Apply room filter to memberships
#[allow(dead_code)] // Matrix sync filtering - will be integrated with sync endpoint
pub fn apply_room_filter(memberships: Vec<Membership>, filter: &MatrixFilter) -> Vec<Membership> {
    if let Some(room_filter) = &filter.room {
        let mut filtered = memberships;

        // Apply room inclusion filter
        if let Some(rooms) = &room_filter.rooms {
            filtered.retain(|m| rooms.contains(&m.room_id));
        }

        // Apply room exclusion filter
        if let Some(not_rooms) = &room_filter.not_rooms {
            filtered.retain(|m| !not_rooms.contains(&m.room_id));
        }

        // Apply include_leave filter
        if !room_filter.include_leave {
            filtered.retain(|m| m.membership != MembershipState::Leave);
        }

        filtered
    } else {
        memberships
    }
}

/// Apply Matrix-compliant event filtering with wildcard support
/// Based on Ruma's RoomEventFilter implementation
pub async fn apply_event_filter(
    events: Vec<Event>,
    filter: &EventFilter,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    let mut filtered = events;

    // Apply event type filtering with wildcard support per Matrix spec
    if let Some(types) = &filter.types
        && !types.is_empty()
    {
        filtered.retain(|event| {
            types
                .iter()
                .any(|type_pattern| match_event_type(&event.event_type, type_pattern))
        });
    }

    // Apply not_types filtering (takes precedence per Matrix spec)
    if let Some(not_types) = &filter.not_types
        && !not_types.is_empty()
    {
        filtered.retain(|event| {
            !not_types
                .iter()
                .any(|type_pattern| match_event_type(&event.event_type, type_pattern))
        });
    }

    // Apply sender filtering
    if let Some(senders) = &filter.senders
        && !senders.is_empty()
    {
        filtered.retain(|event| senders.contains(&event.sender));
    }

    // Apply not_senders filtering
    if let Some(not_senders) = &filter.not_senders
        && !not_senders.is_empty()
    {
        filtered.retain(|event| !not_senders.contains(&event.sender));
    }

    // Apply limit filtering
    if let Some(limit) = filter.limit
        && limit > 0
    {
        filtered.truncate(limit as usize);
    }

    Ok(filtered)
}

/// Matrix-compliant wildcard matching for event types
/// Supports '*' wildcard as specified in Matrix spec
fn match_event_type(event_type: &str, pattern: &str) -> bool {
    if pattern == "*" {
        true
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        event_type.starts_with(prefix)
    } else {
        event_type == pattern
    }
}
