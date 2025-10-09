# SPEC_FEDERATION_01: Complete v2 send_join Implementation

## QA REVIEW RATING: 1/10 - NON-FUNCTIONAL STUB

**CRITICAL STATUS**: The current v2 implementation is a 20-line stub that returns hardcoded fake data. It has ZERO authentication, ZERO validation, ZERO database operations, and is NOT production-ready. This endpoint cannot be used and poses a critical security vulnerability if deployed.

**Completion**: ~2% (20 lines of stub vs 390 lines required)

**What's Correct**:
- ✅ Response format structure (has `state`, `auth_chain`, `event` fields)
- ✅ File location: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v2/send_join/by_room_id/by_event_id.rs`

## Priority
HIGH - Core federation endpoint for room joins

## CRITICAL MISSING COMPONENTS

### 1. Authentication Logic (MISSING - CRITICAL SECURITY ISSUE)
**Impact**: Any server can call this endpoint without verification, allowing malicious room membership forgery.

**Required**:
- X-Matrix authentication header parsing (`parse_x_matrix_auth()` helper)
- Server signature validation via `MatrixSessionService::validate_server_signature()`
- Origin server verification

**Location in v1**: Lines 25-117

### 2. Event Validation (MISSING - CRITICAL)
**Impact**: Malicious or malformed events could be accepted into rooms.

**Required**:
- Event structure validation (sender, state_key, type, membership fields)
- Sender domain verification against origin server
- Event ID path parameter matching
- PDU validation using `PduValidator` with 6-step pipeline

**Location in v1**: Lines 120-244

### 3. Authorization (MISSING - CRITICAL)
**Impact**: Unauthorized servers could join restricted rooms.

**Required**:
- Room existence validation via `RoomRepository::get_by_id()`
- Federation join permission check via `validate_federation_join_allowed()`

**Location in v1**: Lines 174-204

### 4. Cryptographic Signing (MISSING - CRITICAL)
**Impact**: Other Matrix servers will reject events without resident server's signature.

**Required**:
- `sign_join_event()` helper function to add server signature
- Canonical JSON creation and signing
- Signature addition to event.signatures HashMap

**Location in v1**: Lines 247-252, 374-390

### 5. Database Operations (MISSING - CRITICAL)
**Impact**: State is not persisted, joins are not tracked, federation data is lost.

**Required**:
- Store validated event via `EventRepository::create()`
- Create membership record via `MembershipRepository::create()` with proper fields
- Retrieve room state via `EventRepository::get_room_current_state()`
- Build auth chain via `EventRepository::get_auth_chain_for_events()`

**Location in v1**: Lines 255-347

### 6. Function Signature (INCORRECT)
**Current**:
```rust
async fn put(
    Path((_room_id, _event_id)): Path<(String, String)>,
    Json(_payload): Json<Value>,
)
```

**Required**:
```rust
async fn put(
    State(state): State<AppState>,
    Path((room_id, event_id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
)
```

**Missing**: `AppState` injection, `HeaderMap` for authentication

### 7. Imports (INCOMPLETE)
**Current**: Only basic axum and serde_json imports

**Required** (from v1 lines 1-18):
```rust
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::federation::client::FederationClient;
use crate::federation::membership_federation::validate_federation_join_allowed;
use crate::federation::pdu_validator::{PduValidator, PduValidatorParams, ValidationResult};
use crate::state::AppState;
use matryx_entity::types::{Event, Membership, MembershipState};
use matryx_surrealdb::repository::{
    EventRepository, FederationRepository, KeyServerRepository, MembershipRepository,
    RoomRepository,
};
```

### 8. Error Handling (MISSING)
**Current**: Always returns `Ok(Json(...))` with status 200

**Required**: Proper HTTP status codes for:
- 400 BAD_REQUEST: Invalid event structure, validation failures
- 401 UNAUTHORIZED: Missing/invalid X-Matrix authentication
- 403 FORBIDDEN: Federation not allowed, event rejected by validation
- 404 NOT_FOUND: Room not found
- 500 INTERNAL_SERVER_ERROR: Database errors, signing failures

**Location in v1**: Throughout, with proper error mapping

### 9. Response Data (FAKE)
**Current**: Returns empty arrays and hardcoded fake join event

**Required**: Response must contain:
- `state`: Actual current room state PDUs from database
- `auth_chain`: Complete auth chain for room state events
- `event`: The fully signed join event that was persisted

**Location in v1**: Lines 308-367

## Implementation Approach

### RECOMMENDED: Copy v1 and Modify Response Format

The v1 implementation at `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs` is FULLY COMPLETE with all required logic (390 lines).

**Steps**:

1. **Copy entire v1 implementation** (lines 1-390) to v2 file

2. **Change ONLY the response format** (around line 367):

   **v1 returns** (array format):
   ```rust
   let response = json!([
       200,
       {
           "state": room_state,
           "auth_chain": auth_chain
       }
   ]);
   ```

   **v2 must return** (object format):
   ```rust
   let response = json!({
       "state": room_state,
       "auth_chain": auth_chain,
       "event": serde_json::to_value(&stored_event).unwrap_or(json!({}))
   });
   ```

3. **Update doc comment** to reflect v2:
   ```rust
   /// PUT /_matrix/federation/v2/send_join/{roomId}/{eventId}
   ///
   /// Submits a signed join event to a resident server for it to accept it into the room's graph.
   /// This is the v2 API with improved response format compared to v1.
   ```

4. **Optional**: Update logging to indicate v2:
   ```rust
   info!(
       "Successfully processed join event {} for user {} in room {} (v2)",
       event_id, sender, room_id
   );
   ```

### Why This Is Simple

- All complex logic already exists in v1
- Authentication, validation, signing, database operations are proven working
- Only response format differs between v1 and v2
- v2 send_leave follows this exact pattern (see `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v2/send_leave/by_room_id/by_event_id.rs`)

## Definition of Done

The implementation is complete when:

1. ✅ X-Matrix authentication is validated (parse header, verify signature)
2. ✅ Event structure is validated (type, sender, state_key, membership)
3. ✅ Event passes PDU validation (6-step pipeline via `PduValidator`)
4. ✅ Room exists and federation join is authorized
5. ✅ Event is signed by resident server (`sign_join_event()`)
6. ✅ Event is persisted to database (`EventRepository::create()`)
7. ✅ Membership record is created (`MembershipRepository::create()`)
8. ✅ Current room state is retrieved from database
9. ✅ Complete auth chain is built
10. ✅ Response returns v2 format with REAL data: `{"state": [...], "auth_chain": [...], "event": {...}}`
11. ✅ Proper HTTP status codes for all error conditions
12. ✅ Event includes fully signed join event in response
13. ✅ No compilation errors
14. ✅ Logic matches v1 except response format

## Reference Files

- **v1 Implementation** (complete): `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs`
- **v2 Stub** (current): `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v2/send_join/by_room_id/by_event_id.rs`
- **v2 send_leave** (reference pattern): `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v2/send_leave/by_room_id/by_event_id.rs`
- **Matrix Spec**: `/Volumes/samsung_t9/maxtryx/packages/server/tmp/matrix-spec/content/server-server-api.md`

## Complexity Assessment

- **Complexity**: TRIVIAL (copy-paste with response format change)
- **Estimated Time**: 10-15 minutes
- **Lines to Copy**: ~390 lines
- **Lines to Change**: ~10 lines (response format + doc comment)
- **Risk**: MINIMAL (v1 logic is proven working)
