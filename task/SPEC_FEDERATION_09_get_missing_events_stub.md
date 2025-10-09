# SPEC_FEDERATION_09: Fix get_missing_events BFS Algorithm Bug

## Status
**INCOMPLETE** - Critical spec violation bug found. Implementation exists but has incorrect behavior.

## Critical Issue

The BFS traversal in `get_missing_events_traversal()` incorrectly includes `latest_events` in the results. Per the Matrix specification, `latest_events` should only serve as **starting points** for the traversal, NOT be included in the returned events.

**Spec Quote**: "Retrieves previous events that the sender is missing. This is done by doing a breadth-first walk of the `prev_events` for the `latest_events`"

### Current Buggy Behavior

**File**: `packages/server/src/_matrix/federation/v1/get_missing_events/by_room_id.rs:318-370`

```rust
// Add latest events to visited set (they are starting points, not results)
for event_id in latest_events {
    visited.insert(event_id.clone());
}

while !to_visit.is_empty() && result_events.len() < limit {
    let current_batch: Vec<String> = std::mem::take(&mut to_visit);
    
    // Fetch current batch of events
    let events = event_repo
        .get_events_by_ids_with_min_depth(&current_batch, room_id, min_depth)
        .await?;
    
    // Process events and add their prev_events to next batch
    for event in events {
        // BUG: This adds latest_events to results in first iteration!
        if result_events.len() < limit {
            result_events.push(pdu);
        }
    }
}
```

**Problem**: First iteration fetches `latest_events` and adds them to `result_events`, violating the spec.

### Correct Behavior (Synapse Reference)

**File**: `tmp/synapse/synapse/storage/databases/main/event_federation.py:1649-1675`

```python
seen_events = set(earliest_events)
front = set(latest_events) - seen_events
event_results: List[str] = []

while front and len(event_results) < limit:
    new_front = set()
    for event_id in front:
        # Query for PREV_EVENT_IDs, not the event itself
        txn.execute(query, (event_id, limit - len(event_results)))
        new_results = {t[0] for t in txn} - seen_events
        
        new_front |= new_results
        seen_events |= new_results
        event_results.extend(new_results)  # Only prev_events added
    
    front = new_front
```

**Key Difference**: Synapse queries for `prev_event_id` values and adds those to results, never the events in `front` themselves.

---

## Required Fix

### Algorithm Correction

The BFS must be rewritten to:

1. **First iteration**: Fetch `latest_events` from database to get their `prev_events` lists
2. **Do NOT add** `latest_events` to results
3. **Extract** all `prev_event_ids` from `latest_events`
4. **Start BFS** from those `prev_event_ids`, fetching them and adding to results
5. **Continue** traversing backwards through prev_events until limit or earliest_events reached

### Suggested Implementation

```rust
async fn get_missing_events_traversal(
    state: &AppState,
    room_id: &str,
    latest_events: &[String],
    earliest_events: &[String],
    limit: usize,
    min_depth: i64,
) -> Result<Vec<PDU>, Box<dyn std::error::Error + Send + Sync>> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut to_visit: Vec<String> = Vec::new();
    let mut result_events = Vec::new();
    let earliest_set: HashSet<String> = earliest_events.iter().cloned().collect();
    
    // Mark latest_events as visited (they are starting points only)
    for event_id in latest_events {
        visited.insert(event_id.clone());
    }
    
    // STEP 1: Fetch latest_events to get their prev_events (don't add to results)
    let event_repo = EventRepository::new(state.db.clone());
    let start_events = event_repo
        .get_events_by_ids_with_min_depth(latest_events, room_id, min_depth)
        .await?;
    
    // Extract prev_events from latest_events as the true starting point
    for event in start_events {
        if let Some(prev_events) = &event.prev_events {
            for prev_event_id in prev_events {
                if !visited.contains(prev_event_id) && !earliest_set.contains(prev_event_id) {
                    visited.insert(prev_event_id.to_string());
                    to_visit.push(prev_event_id.to_string());
                }
            }
        }
    }
    
    // STEP 2: BFS traversal starting from prev_events (these DO get added to results)
    while !to_visit.is_empty() && result_events.len() < limit {
        let current_batch: Vec<String> = std::mem::take(&mut to_visit);
        
        let events = event_repo
            .get_events_by_ids_with_min_depth(&current_batch, room_id, min_depth)
            .await?;
        
        for event in events {
            // Skip if in earliest_events boundary
            if earliest_set.contains(&event.event_id) {
                continue;
            }
            
            // Queue this event's prev_events for next iteration
            if let Some(prev_events) = &event.prev_events {
                for prev_event_id in prev_events {
                    if !visited.contains(prev_event_id) && !earliest_set.contains(prev_event_id) {
                        visited.insert(prev_event_id.to_string());
                        to_visit.push(prev_event_id.to_string());
                    }
                }
            }
            
            // Add event to results (NOW this is correct)
            if result_events.len() < limit {
                let depth = event.depth.ok_or("Event missing required depth field")?;
                let prev_events = event.prev_events.clone().unwrap_or_default();
                let auth_events = event.auth_events.clone().unwrap_or_default();
                
                let pdu = PDU {
                    event_id: event.event_id.clone(),
                    room_id: event.room_id.clone(),
                    sender: event.sender.clone(),
                    origin_server_ts: event.origin_server_ts,
                    event_type: event.event_type.clone(),
                    content: event.content.clone(),
                    state_key: event.state_key.clone(),
                    prev_events,
                    auth_events,
                    depth,
                    signatures: event.signatures.clone().unwrap_or_default(),
                    hashes: event.hashes.clone().unwrap_or_default(),
                    unsigned: event.unsigned.clone().and_then(|v| serde_json::from_value(v).ok()),
                };
                result_events.push(pdu);
            }
        }
    }
    
    // Sort by depth descending (most recent first) then by origin_server_ts
    result_events.sort_by(|a, b| match b.depth.cmp(&a.depth) {
        std::cmp::Ordering::Equal => b.origin_server_ts.cmp(&a.origin_server_ts),
        other => other,
    });
    
    Ok(result_events)
}
```

---

## Important Security Enhancement

### Event Visibility Filtering (Required for Production)

The current implementation returns all events without checking if the requesting server has permission to see them based on room history visibility settings. This is a **privacy and security issue**.

**Required**: Add server-side event visibility filtering before returning results.

**Implementation Location**: Before final response in `post()` handler

**Reference**: Synapse's `filter_events_for_server()` in `tmp/synapse/synapse/handlers/federation.py:1408-1422`

### Filtering Requirements

1. Check room `history_visibility` setting (world_readable, invited, joined, shared)
2. Verify requesting server's membership status at each event's depth
3. Redact or exclude events the server shouldn't see based on visibility rules
4. Handle erased senders (GDPR compliance)
5. Filter out partial state events from remote servers

---

## Optional Refinements

### 1. Reduce Maximum Limit (Recommended)

**Current**: Allows up to 100 events  
**Synapse**: Hard-caps at 20 events

**Rationale**: Synapse's conservative 20-event limit prevents excessive data transfer and aligns with production homeserver behavior.

**Change**: Line ~227-231 in `by_room_id.rs`

```rust
// Current (allows 100)
if limit == 0 || limit > 100 {
    return Err(StatusCode::BAD_REQUEST);
}

// Recommended (cap at 20)
let limit = if limit == 0 {
    10  // Default
} else {
    std::cmp::min(limit as usize, 20)  // Cap at 20 like Synapse
};
```

### 2. State Event Filtering (Optional)

**Current**: Returns all events (state and non-state)  
**Synapse**: Excludes state events with `NOT is_state` filter

**Consideration**: Matrix spec doesn't explicitly require this, but Synapse does it. Research whether state events in backfill responses cause issues with event graph reconstruction.

---

## Definition of Done

- [ ] **CRITICAL**: BFS algorithm fixed to exclude latest_events from results
- [ ] **CRITICAL**: Event visibility filtering implemented for security
- [ ] Optional: Limit reduced from 100 to 20 events
- [ ] Optional: State event filtering added
- [ ] Verify fix with integration test comparing results with/without the bug
- [ ] Confirm compliance with Matrix spec behavior

---

## Testing Verification

Create a simple test case:

```
latest_events = [E5]
E5.prev_events = [E3, E4]
E3.prev_events = [E1]
E4.prev_events = [E2]
```

**Buggy behavior**: Returns [E5, E4, E3, E2, E1]  
**Correct behavior**: Returns [E4, E3, E2, E1] (E5 excluded)

---

## Summary

The implementation is comprehensive with excellent authentication, authorization, and input validation. However, it has a **critical spec violation** where `latest_events` are incorrectly included in results. Additionally, event visibility filtering is missing, which is a security/privacy concern for production use.

**Priority**: **HIGH** - Spec violation affecting federation correctness
