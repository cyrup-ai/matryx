use std::hash::{DefaultHasher, Hash, Hasher};

use crate::cache::lazy_loading_cache::LazyLoadingCache;
use crate::state::AppState;
use matryx_entity::types::Event;

/// Cache-aware lazy loading filter that reuses cached results for repeated requests
pub async fn apply_cache_aware_lazy_loading_filter(
    events: Vec<Event>,
    room_id: &str,
    user_id: &str,
    state: &AppState,
    include_redundant_members: bool,
    lazy_cache: &LazyLoadingCache,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    if include_redundant_members {
        return Ok(events);
    }

    // Extract timeline senders for cache key calculation
    let timeline_senders: Vec<String> = events
        .iter()
        .filter(|e| e.event_type != "m.room.member")
        .map(|e| e.sender.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let filter_hash = calculate_lazy_loading_hash(room_id, user_id, &timeline_senders);

    // Check for cached results first (cache-aware filtering)
    if let Some(cached_events) =
        lazy_cache.get_cached_membership_events(room_id, &filter_hash).await
    {
        // Merge cached membership events with current non-membership events
        let non_membership_events: Vec<Event> =
            events.into_iter().filter(|e| e.event_type != "m.room.member").collect();

        let mut result_events = cached_events;
        result_events.extend(non_membership_events);
        return Ok(result_events);
    }

    // No cached results found, proceed with full processing
    apply_lazy_loading_filter_enhanced(
        events,
        room_id,
        user_id,
        state,
        include_redundant_members,
        lazy_cache,
    )
    .await
}

/// Enhanced Matrix-compliant lazy loading with SurrealDB LiveQuery and cache optimization
async fn apply_lazy_loading_filter_enhanced(
    events: Vec<Event>,
    room_id: &str,
    user_id: &str,
    state: &AppState,
    include_redundant_members: bool,
    lazy_cache: &LazyLoadingCache,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    if include_redundant_members {
        return Ok(events);
    }

    // Start performance tracking
    let start_time = std::time::Instant::now();
    let original_member_count = events.iter().filter(|e| e.event_type == "m.room.member").count();

    // Extract timeline senders for essential member calculation
    let timeline_senders: Vec<String> = events
        .iter()
        .filter(|e| e.event_type != "m.room.member")
        .map(|e| e.sender.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Get essential members using optimized cache + database query with LiveQuery invalidation
    let membership_repo =
        matryx_surrealdb::repository::membership::MembershipRepository::new(state.db.clone());

    // Ensure real-time cache invalidation is active for this room
    if let Err(e) = lazy_cache.start_live_invalidation(room_id, &membership_repo).await {
        tracing::warn!(room_id = %room_id, error = %e, "Failed to start live cache invalidation, falling back to TTL-only caching");
    }

    let essential_members = lazy_cache
        .get_essential_members_cached(room_id, user_id, &timeline_senders, &membership_repo)
        .await?;

    // Filter events based on essential members
    let filtered_events: Vec<Event> = events
        .into_iter()
        .filter(|event| {
            if event.event_type == "m.room.member" {
                // Only include membership events for essential members
                event
                    .state_key
                    .as_ref()
                    .map(|state_key| essential_members.contains(state_key))
                    .unwrap_or(false)
            } else {
                // Always include non-membership events
                true
            }
        })
        .collect();

    // Cache the filtered results for future requests
    let filter_hash = calculate_lazy_loading_hash(room_id, user_id, &timeline_senders);
    lazy_cache
        .cache_membership_events(room_id, &filter_hash, filtered_events.clone())
        .await;

    // Record performance metrics
    let processing_time = start_time.elapsed();
    let filtered_member_count =
        filtered_events.iter().filter(|e| e.event_type == "m.room.member").count();
    let members_filtered_out = original_member_count.saturating_sub(filtered_member_count);

    // Log performance for monitoring
    tracing::debug!(
        room_id = %room_id,
        user_id = %user_id,
        original_members = original_member_count,
        essential_members = essential_members.len(),
        filtered_members = filtered_member_count,
        members_filtered_out = members_filtered_out,
        processing_time_ms = processing_time.as_millis(),
        "Lazy loading filter applied with LiveQuery optimization"
    );

    // Record metrics for dashboard
    if let Some(metrics) = state.lazy_loading_metrics.as_ref() {
        let cache_hit = !essential_members.is_empty(); // Simplified cache hit detection
        let _ = metrics
            .record_operation(room_id, processing_time, cache_hit, members_filtered_out as u64)
            .await;
    }

    Ok(filtered_events)
}

/// Implement Matrix-compliant lazy loading per specification (fallback implementation)
/// Based on spec lines 743-753: lazy loading requirements
pub async fn apply_lazy_loading_filter(
    events: Vec<Event>,
    _room_id: &str,
    user_id: &str,
    _state: &AppState,
    include_redundant_members: bool,
) -> Result<Vec<Event>, Box<dyn std::error::Error + Send + Sync>> {
    if include_redundant_members {
        return Ok(events);
    }

    let mut filtered = Vec::new();
    let mut seen_senders = std::collections::HashSet::new();

    for event in events {
        if event.event_type == "m.room.member" {
            // For lazy loading, only include membership events:
            // 1. For the requesting user
            // 2. For senders of other timeline events
            // 3. For membership changes during this sync
            if event.state_key.as_ref() == Some(&user_id.to_string())
                || !seen_senders.contains(&event.sender)
            {
                seen_senders.insert(event.sender.clone());
                filtered.push(event);
            }
        } else {
            // Always include non-membership events
            seen_senders.insert(event.sender.clone());
            filtered.push(event);
        }
    }

    Ok(filtered)
}

/// Calculate hash for lazy loading cache key
fn calculate_lazy_loading_hash(
    room_id: &str,
    user_id: &str,
    timeline_senders: &[String],
) -> String {
    let mut hasher = DefaultHasher::new();
    room_id.hash(&mut hasher);
    user_id.hash(&mut hasher);
    timeline_senders.hash(&mut hasher);

    format!("{:x}", hasher.finish())
}
