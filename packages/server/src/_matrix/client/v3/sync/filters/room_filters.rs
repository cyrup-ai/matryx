use super::basic_filters::apply_event_filter;
use super::lazy_loading::{apply_cache_aware_lazy_loading_filter, apply_lazy_loading_filter};
use super::url_filters::apply_contains_url_filter;
use crate::cache::lazy_loading_cache::LazyLoadingCache;
use crate::state::AppState;
use matryx_entity::filter::RoomEventFilter;
use matryx_entity::types::Event;

/// Apply room event filtering including contains_url and lazy loading
/// Based on Matrix spec lazy-loading requirements
pub async fn apply_room_event_filter(
    events: Vec<Event>,
    filter: &RoomEventFilter,
    room_id: &str,
    user_id: &str,
    state: &AppState,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    // Apply base event filtering first
    let mut filtered = apply_event_filter(events, &filter.base).await?;

    // Apply contains_url filtering per Matrix spec
    if let Some(contains_url) = filter.contains_url {
        filtered = apply_contains_url_filter(filtered, contains_url).await?;
    }

    // Apply enhanced lazy loading with SurrealDB LiveQuery optimization per Matrix spec
    if filter.lazy_load_members {
        // Use enhanced lazy loading with real-time cache invalidation if available
        if let Some(lazy_cache) = &state.lazy_loading_cache {
            filtered = apply_cache_aware_lazy_loading_filter(
                filtered,
                room_id,
                user_id,
                state,
                filter.include_redundant_members,
                lazy_cache,
            )
            .await?;
        } else {
            // Fallback to basic lazy loading if enhanced cache not available
            filtered = apply_lazy_loading_filter(
                filtered,
                room_id,
                user_id,
                state,
                filter.include_redundant_members,
            )
            .await?;
        }
    }

    Ok(filtered)
}
