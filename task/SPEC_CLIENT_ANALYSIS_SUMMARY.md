# Matrix Client-Server API - Outstanding Implementation Tasks

## Overview
This document tracks incomplete Matrix Client-Server API implementations requiring completion.

**Review Date**: 2025-10-09  
**QA Rating**: 3/10 (Infrastructure exists but not integrated)

---

## 1. Room Messages Pagination (HIGH PRIORITY)

**Rating**: 2/10 - Stub implementation, minimal database support

**Endpoint**: `GET /_matrix/client/v3/rooms/{roomId}/messages`  
**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/rooms/by_room_id/messages.rs`

**Current Status**:
- ✗ API endpoint is a stub returning `{"chunk": [], "start": "t0", "end": "t1"}`
- ✗ No AppState parameter - completely disconnected from database
- ✓ Basic `get_room_messages()` exists in RoomRepository (line 2742)
- ✗ Missing pagination token generation and parsing
- ✗ Missing direction (forward/backward) support
- ✗ Missing filter parameter support

**Required Implementation**:

1. **Update API endpoint signature**:
   ```rust
   pub async fn get(
       State(state): State<AppState>,
       Path(room_id): Path<String>,
       Query(params): Query<MessagesQueryParams>,
   ) -> Result<Json<MessagesResponse>, StatusCode>
   ```

2. **Add query parameters struct**:
   - `from`: Pagination token (optional)
   - `to`: Ending token (optional) 
   - `dir`: Direction ("b" backward, "f" forward)
   - `limit`: Max events (default 10)
   - `filter`: Event filter (optional)

3. **Enhance database layer** (`packages/surrealdb/src/repository/room.rs`):
   - Add pagination token type (format: `t{timestamp}_{event_id}`)
   - Implement token parsing and generation
   - Add direction-aware queries (ORDER BY ASC/DESC based on direction)
   - Support filter parameter
   - Return proper pagination tokens in response

4. **Integration**:
   - Call `room_operations.room_repo.get_room_messages_paginated()` with parameters
   - Generate response with actual events and proper `start`/`end` tokens
   - Handle edge cases (no more events, invalid tokens, room access validation)

---

## 2. Read Markers (MEDIUM PRIORITY)

**Rating**: 3/10 - Database functions exist but API doesn't use them

**Endpoint**: `POST /_matrix/client/v3/rooms/{roomId}/read_markers`  
**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs`

**Current Status**:
- ✗ API endpoint stub returns `{}` without processing request
- ✓ Database function `update_read_marker()` exists (event.rs:1766)
- ✓ Database function `get_unread_events()` exists (event.rs:1788)
- ✗ API endpoint not connected to database layer

**Required Implementation**:

1. **Update API endpoint**:
   ```rust
   pub async fn post(
       State(state): State<AppState>,
       Path(room_id): Path<String>,
       Json(payload): Json<ReadMarkersRequest>,
   ) -> Result<Json<Value>, StatusCode>
   ```

2. **Parse request body**:
   - `m.fully_read`: Fully read marker event ID
   - `m.read`: Public read receipt (optional)
   - `m.read.private`: Private read receipt (optional)

3. **Database integration**:
   - Call `state.room_operations.event_repo.update_read_marker()` for `m.fully_read`
   - If `m.read` or `m.read.private` provided, delegate to receipt handler
   - Store marker in account_data with type "m.fully_read"

4. **Sync integration**:
   - Ensure read markers appear in `/sync` response account_data section
   - Return proper empty response on success

---

## 3. Presence (LOW PRIORITY)

**Rating**: 4/10 - Full repository exists but not integrated

**Endpoints**:
- `GET /_matrix/client/v3/presence/{userId}/status`
- `PUT /_matrix/client/v3/presence/{userId}/status`

**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/presence/by_user_id/status.rs`

**Current Status**:
- ✗ GET returns hardcoded `{"presence": "online", "last_active_ago": 0}`
- ✗ PUT accepts but ignores request
- ✓ **Complete PresenceRepository exists** (`packages/surrealdb/src/repository/presence.rs`)
  - ✓ `update_user_presence()` - Store presence updates
  - ✓ `get_user_presence()` - Retrieve user presence
  - ✓ `set_user_online/offline/unavailable()` - Helper methods
  - ✓ `subscribe_to_user_presence()` - LiveQuery support
  - ✓ `get_presence_events_for_users()` - Batch retrieval
  - ✓ Complete PresenceEvent and PresenceState types
- ✗ **PresenceRepository NOT in AppState** (state.rs has no presence_repo field)

**Required Implementation**:

1. **Add PresenceRepository to AppState** (`packages/server/src/state.rs`):
   ```rust
   pub struct AppState {
       // ... existing fields
       pub presence_repo: Arc<PresenceRepository>,
   }
   ```
   - Initialize in `AppState::new()` and `with_lazy_loading_optimization()`

2. **Update GET endpoint**:
   ```rust
   pub async fn get(
       State(state): State<AppState>,
       Path(user_id): Path<String>,
   ) -> Result<Json<PresenceResponse>, StatusCode>
   ```
   - Call `state.presence_repo.get_user_presence(&user_id)`
   - Calculate `last_active_ago` from stored timestamp
   - Return actual presence state or 404 if not found

3. **Update PUT endpoint**:
   ```rust
   pub async fn put(
       State(state): State<AppState>,
       Path(user_id): Path<String>,
       Json(payload): Json<PresenceRequest>,
   ) -> Result<Json<Value>, StatusCode>
   ```
   - Validate user_id matches authenticated user
   - Call appropriate helper: `set_user_online/offline/unavailable()`
   - Return empty response on success

4. **Sync integration**:
   - Include presence updates in `/sync` response
   - Support presence lists and subscriptions (future enhancement)

---

## Implementation Priority

1. **Read Markers** (easiest) - Database functions ready, just need API connection
2. **Room Messages** (critical) - Most important for UX, requires pagination logic
3. **Presence** (optional) - Complete repo exists, just needs AppState integration

## Summary

All three features are **INCOMPLETE**. Main issues:
- API endpoints are stubs with no AppState integration
- Database layer exists but disconnected from HTTP layer
- Need to wire up existing repositories to endpoint handlers
- Presence has full infrastructure but is completely unused

**Next Steps**: Focus on API-to-database integration rather than building new infrastructure.
