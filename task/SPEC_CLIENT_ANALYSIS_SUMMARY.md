# Matrix Client-Server API - Outstanding Implementation Tasks

## Overview
This document tracks the required filter parameter implementation for room messages pagination.

**Review Date**: 2025-10-09  
**QA Rating**: 9/10 → Target 10/10 with filter implementation  
**Status**: Filter parameter implementation REQUIRED for completion

---

## COMPLETED FEATURES ✅

### 1. Room Messages Pagination - COMPLETE (9/10)
**Endpoint**: `GET /_matrix/client/v3/rooms/{roomId}/messages`  
**Status**: Production-ready with all core features implemented

✅ API endpoint fully integrated with AppState  
✅ Complete authentication and authorization  
✅ All query parameters defined (from, to, dir, limit, filter)  
✅ Database layer with pagination token support  
✅ Token parsing and generation (`t{timestamp}_{event_id}` format)  
✅ Direction support (forward/backward with proper ORDER BY)  
✅ Proper error handling and validation  
✅ Registered in routing layer (main.rs:436)  

⚠️ **Minor Gap**: `filter` parameter accepted but not used in database query

### 2. Read Markers - COMPLETE (10/10)
**Endpoint**: `POST /_matrix/client/v3/rooms/{roomId}/read_markers`  
**Status**: Production-ready

✅ API endpoint fully integrated with AppState  
✅ Complete authentication and request parsing  
✅ Database function `mark_event_as_read()` fully implemented  
✅ Proper handling of m.fully_read, m.read, m.read.private  
✅ Registered in routing layer (main.rs:474)  

### 3. Presence - COMPLETE (10/10)
**Endpoints**: `GET/PUT /_matrix/client/v3/presence/{userId}/status`  
**Status**: Production-ready

✅ Both GET and PUT endpoints fully implemented  
✅ PresenceRepository in AppState (line 60 of state.rs)  
✅ Repository initialized in both AppState constructors  
✅ Complete database layer with all methods  
✅ Full authentication and validation  
✅ Registered in routing layer (main.rs:417, 494)  

---

## OUTSTANDING ITEM - REQUIRED

### Filter Parameter Support - REQUIRED IMPLEMENTATION

**Priority**: HIGH - Must be implemented for 10/10 completion

**Specification Reference**: `/tmp/matrix-spec/data/api/client-server/message_pagination.yaml` lines 101-106
```yaml
- in: query
  name: filter
  description: A JSON RoomEventFilter to filter returned events with.
  example: '{"contains_url":true}'
  schema:
    type: string
```

**Current State**:
- Filter parameter defined in `MessagesQueryParams` struct (line 24)
- Parameter accepted by API but not passed to database
- No filter processing in `get_room_messages_paginated()`

**Required Implementation Steps**:

#### 1. Define RoomEventFilter Type
**File**: `packages/entity/src/types/room_event_filter.rs` (NEW FILE)

Create a complete RoomEventFilter struct according to Matrix spec:
```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomEventFilter {
    /// Maximum number of events to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    
    /// A list of sender IDs to exclude
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_senders: Option<Vec<String>>,
    
    /// A list of event types to exclude
    #[serde(skip_serializing_if = "Option::is_none")]
    pub not_types: Option<Vec<String>>,
    
    /// A list of senders IDs to include
    #[serde(skip_serializing_if = "Option::is_none")]
    pub senders: Option<Vec<String>>,
    
    /// A list of event types to include
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,
    
    /// Whether to include events with a URL in their content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contains_url: Option<bool>,
    
    /// Whether to include redundant member events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_redundant_members: Option<bool>,
    
    /// Whether to enable lazy-loading of room members
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lazy_load_members: Option<bool>,
}
```

Export in `packages/entity/src/types/mod.rs`:
```rust
pub mod room_event_filter;
pub use room_event_filter::RoomEventFilter;
```

#### 2. Parse Filter in API Endpoint
**File**: `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs`

After line 64 (after room validation), add:
```rust
// Parse filter parameter if provided
let filter = if let Some(filter_str) = &params.filter {
    match serde_json::from_str::<RoomEventFilter>(filter_str) {
        Ok(f) => Some(f),
        Err(e) => {
            warn!("Invalid filter JSON: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    }
} else {
    None
};
```

#### 3. Update Database Function Signature
**File**: `packages/surrealdb/src/repository/room.rs` line 2776

Change function signature:
```rust
pub async fn get_room_messages_paginated(
    &self,
    room_id: &str,
    from_token: Option<&str>,
    to_token: Option<&str>,
    direction: &str,
    limit: u32,
    filter: Option<&RoomEventFilter>,  // ADD THIS PARAMETER
) -> Result<(Vec<Event>, String, String), RepositoryError>
```

#### 4. Apply Filter in Database Query
**File**: `packages/surrealdb/src/repository/room.rs`

After line 2820 (after base query construction), add filter logic:
```rust
// Apply filter conditions if provided
if let Some(filter) = filter {
    // Filter by event types
    if let Some(types) = &filter.types {
        if !types.is_empty() {
            let types_str = types.iter()
                .map(|t| format!("'{}'", t))
                .collect::<Vec<_>>()
                .join(", ");
            query.push_str(&format!(" AND event_type IN [{}]", types_str));
        }
    }
    
    // Exclude event types
    if let Some(not_types) = &filter.not_types {
        if !not_types.is_empty() {
            let not_types_str = not_types.iter()
                .map(|t| format!("'{}'", t))
                .collect::<Vec<_>>()
                .join(", ");
            query.push_str(&format!(" AND event_type NOT IN [{}]", not_types_str));
        }
    }
    
    // Filter by senders
    if let Some(senders) = &filter.senders {
        if !senders.is_empty() {
            let senders_str = senders.iter()
                .map(|s| format!("'{}'", s))
                .collect::<Vec<_>>()
                .join(", ");
            query.push_str(&format!(" AND sender IN [{}]", senders_str));
        }
    }
    
    // Exclude senders
    if let Some(not_senders) = &filter.not_senders {
        if !not_senders.is_empty() {
            let not_senders_str = not_senders.iter()
                .map(|s| format!("'{}'", s))
                .collect::<Vec<_>>()
                .join(", ");
            query.push_str(&format!(" AND sender NOT IN [{}]", not_senders_str));
        }
    }
    
    // Filter by contains_url
    if let Some(contains_url) = filter.contains_url {
        if contains_url {
            query.push_str(" AND content.url != NONE");
        }
    }
}
```

#### 5. Update API Endpoint Call
**File**: `packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs` line 117

Change the database call to pass filter:
```rust
let (events, start_token, end_token) = room_repo
    .get_room_messages_paginated(
        &room_id,
        params.from.as_deref(),
        params.to.as_deref(),
        &params.dir,
        params.limit,
        filter.as_ref(),  // ADD THIS
    )
    .await
    .map_err(|e| {
        error!("Failed to get room messages: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
```

#### 6. Add Tests
**File**: `packages/surrealdb/src/repository/room_test.rs`

Add comprehensive filter tests:
```rust
#[tokio::test]
async fn test_messages_pagination_filter_types() {
    // Test filtering by event types
}

#[tokio::test]
async fn test_messages_pagination_filter_senders() {
    // Test filtering by senders
}

#[tokio::test]
async fn test_messages_pagination_filter_contains_url() {
    // Test contains_url filter
}

#[tokio::test]
async fn test_messages_pagination_filter_combined() {
    // Test multiple filter criteria together
}
```

**Expected Outcome**: All filter parameters from the Matrix specification are properly implemented and functional, bringing the implementation to 10/10 completion.

---

## Summary

**Implementation Status**: 99% Complete → Target 100% with filter implementation  
**Production Readiness**: Deployable but filter support REQUIRED for 10/10 completion  

All three features are fully functional with proper:
- AppState integration ✅
- Authentication ✅
- Database operations ✅
- Error handling ✅
- Route registration ✅

**Outstanding**: Filter parameter implementation in room messages pagination (see detailed steps above)

**Acceptance Criteria for 10/10**:
1. RoomEventFilter type defined and exported
2. Filter JSON parsing in API endpoint with error handling
3. Database function accepts filter parameter
4. All filter criteria applied in SQL query (types, senders, not_types, not_senders, contains_url)
5. Comprehensive test coverage for filter functionality
6. No use of unwrap() or expect() in implementation
