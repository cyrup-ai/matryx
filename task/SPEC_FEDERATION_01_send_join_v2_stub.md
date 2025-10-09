# SPEC_FEDERATION_01: Complete v2 send_join Implementation

## Status
INCOMPLETE - Currently a stub returning hardcoded JSON

## Priority
HIGH - Core federation endpoint for room joins

## Core Objective

Implement the Matrix Federation API v2 send_join endpoint by adapting the existing v1 implementation. The v2 endpoint accepts a join event from a remote server that wants to join a room hosted on this homeserver, validates it, signs it, and returns the full room state and auth chain.

**Key Discovery**: The v1 send_join implementation at [`packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs`](../packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs) is **FULLY COMPLETE** with all required logic (389 lines). The v2 implementation is **TRIVIAL** - just copy the v1 logic and change the response format.

## Specification Reference

**Endpoint**: `PUT /_matrix/federation/v2/send_join/{roomId}/{eventId}`

**Matrix Spec Location**: [`packages/server/tmp/matrix-spec/content/server-server-api.md`](../packages/server/tmp/matrix-spec/content/server-server-api.md)

### Response Format Difference (v1 vs v2)

**v1 Response** (array format - line 367 of v1 implementation):
```json
[
  200,
  {
    "state": [<PDUs>],
    "auth_chain": [<PDUs>]
  }
]
```

**v2 Response** (object format - required):
```json
{
  "state": [<PDUs>],
  "auth_chain": [<PDUs>],
  "event": <signed_join_event>
}
```

The v2 response includes the fully signed join event that was accepted into the room.

## Current Implementation

**File**: [`packages/server/src/_matrix/federation/v2/send_join/by_room_id/by_event_id.rs`](../packages/server/src/_matrix/federation/v2/send_join/by_room_id/by_event_id.rs)

Current stub (20 lines):
```rust
use axum::{Json, extract::Path, http::StatusCode};
use serde_json::{Value, json};

/// PUT /_matrix/federation/v2/send_join/{roomId}/{eventId}
pub async fn put(
    Path((_room_id, _event_id)): Path<(String, String)>,
    Json(_payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({
        "state": [],
        "auth_chain": [],
        "event": {
            "type": "m.room.member",
            "state_key": "@joiner:example.com",
            "content": {
                "membership": "join"
            }
        }
    })))
}
```

## Complete v1 Implementation Reference

The v1 implementation at [`packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs`](../packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs) provides ALL the logic needed:

### 1. X-Matrix Authentication (lines 18-76)
- Helper struct `XMatrixAuth` (lines 18-22)
- Parser function `parse_x_matrix_auth()` (lines 25-76) - extracts origin, key_id, signature from Authorization header

### 2. Request Handler Structure (lines 78-372)
- Signature: `async fn put(State(state): State<AppState>, Path((room_id, event_id)): Path<(String, String)>, headers: HeaderMap, Json(payload): Json<Value>) -> Result<Json<Value>, StatusCode>`
- All required imports already present (lines 1-15)

### 3. Complete Validation Pipeline (lines 93-244)
- X-Matrix authentication header parsing (lines 93-96)
- Server signature validation (lines 102-117)  
- Event structure validation (lines 120-171):
  - Validates sender, state_key, event_type fields
  - Ensures event_type is "m.room.member"
  - Ensures sender equals state_key
  - Validates membership is "join"
  - Validates user belongs to requesting server
  - Validates event_id matches path parameter
- Room existence validation (lines 174-185)
- Federation permission check via `validate_federation_join_allowed()` (lines 195-204)
- **PDU validation** using `PduValidator` with 6-step validation pipeline (lines 207-244)

### 4. Event Signing (lines 247-252)
- Uses helper function `sign_join_event()` defined at lines 374-390
- Adds resident server's signature to the event

### 5. Database Persistence (lines 255-305)
- Stores validated and signed event via `EventRepository::create()` (lines 255-261)
- Creates/updates membership record via `MembershipRepository::create()` (lines 264-305)

### 6. Room State and Auth Chain Retrieval (lines 308-347)
- Gets current room state via `EventRepository::get_room_current_state()` (lines 308-317)
- Gets auth chain via `EventRepository::get_auth_chain_for_events()` (lines 330-337)
- Converts events to JSON format for response (lines 320-327, 340-344)

### 7. Response Building (lines 350-367)
- **This is the ONLY part that needs to change for v2**
- v1 uses array format: `json!([200, {"state": room_state, "auth_chain": auth_chain}])`

### Key Helper Function: sign_join_event()

Located at lines 374-390 of v1 implementation:
```rust
async fn sign_join_event(
    state: &AppState,
    mut event: Event,
) -> Result<Event, Box<dyn std::error::Error + Send + Sync>> {
    // Gets server signing key
    // Creates canonical JSON
    // Signs the event
    // Adds signature to event.signatures HashMap
    // Returns signed event
}
```

This function is used by v1 and should be **copied verbatim** to v2.

## Existing Infrastructure Already Available

All required components are implemented and ready to use:

### Authentication & Authorization
- **`parse_x_matrix_auth()`** - Parses X-Matrix authentication headers
- **`MatrixSessionService::validate_server_signature()`** - Validates server signatures
- **`validate_federation_join_allowed()`** from [`packages/server/src/federation/membership_federation.rs`](../packages/server/src/federation/membership_federation.rs)

### Event Validation
- **`PduValidator`** from [`packages/server/src/federation/pdu_validator.rs`](../packages/server/src/federation/pdu_validator.rs) - Implements 6-step PDU validation pipeline per Matrix spec
- **`PduValidatorParams`** - Configuration struct for validator
- **`ValidationResult`** enum - Returns Valid, SoftFailed, or Rejected

### Database Repositories
- **`EventRepository::create()`** - Stores events
- **`EventRepository::get_room_current_state()`** - Retrieves current room state (excluding specified event)
- **`EventRepository::get_auth_chain_for_events()`** - Builds complete auth chain
- **`RoomRepository::get_by_id()`** - Fetches room by ID
- **`MembershipRepository::create()`** - Creates/updates membership records
- **`FederationRepository`** - Federation-specific queries
- **`KeyServerRepository`** - Server key management

All repositories located in [`packages/surrealdb/src/repository/`](../packages/surrealdb/src/repository/)

### Federation Support
- **`FederationClient`** from [`packages/server/src/federation/client.rs`](../packages/server/src/federation/client.rs)
- **`MatrixDnsResolver`** from [`packages/server/src/federation/dns_resolver.rs`](../packages/server/src/federation/dns_resolver.rs)

### Entity Types
- **`Event`** from [`packages/entity/src/types/event.rs`](../packages/entity/src/types/event.rs) - Matrix PDU structure with signatures, hashes, auth_events, etc.
- **`Membership`** from [`packages/entity/src/types/membership.rs`](../packages/entity/src/types/) - Membership state tracking
- **`MembershipState`** enum - Join, Leave, Invite, Ban, Knock

## What Needs to Change

### Single File Edit Required

**File**: [`packages/server/src/_matrix/federation/v2/send_join/by_room_id/by_event_id.rs`](../packages/server/src/_matrix/federation/v2/send_join/by_room_id/by_event_id.rs)

### Implementation Steps

1. **Copy the entire v1 implementation** from [`packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs`](../packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs) (lines 1-390)

2. **Change ONLY the response format** (around line 367 in v1):

**Replace this v1 response:**
```rust
// Build response in the Matrix v1 format (array format)
let response = json!([
    200,
    {
        "state": room_state,
        "auth_chain": auth_chain
    }
]);
```

**With this v2 response:**
```rust
// Build response in the Matrix v2 format (direct object, not array)
let response = json!({
    "state": room_state,
    "auth_chain": auth_chain,
    "event": serde_json::to_value(&stored_event).unwrap_or(json!({}))
});
```

3. **Update the doc comment** to reflect v2 API:
```rust
/// PUT /_matrix/federation/v2/send_join/{roomId}/{eventId}
///
/// Submits a signed join event to a resident server for it to accept it into the room's graph.
/// This is the v2 API with improved response format compared to v1.
```

4. **Update logging** to indicate v2 (optional but recommended):
```rust
info!(
    "Successfully processed join event {} for user {} in room {} (v2)",
    event_id, sender, room_id
);
```

### Required Imports

All imports from v1 should be copied as-is (lines 1-15 of v1):
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

## Comparison with v2 send_leave

The v2 send_leave implementation at [`packages/server/src/_matrix/federation/v2/send_leave/by_room_id/by_event_id.rs`](../packages/server/src/_matrix/federation/v2/send_leave/by_room_id/by_event_id.rs) follows the exact same pattern:
- Copies all v1 send_leave logic (385 lines)
- Changes response format from v1 array to v2 object
- v2 send_leave returns empty object: `json!({})` (line 338)
- v2 send_join should return object with state, auth_chain, and event

## Definition of Done

The v2 send_join endpoint is complete when:

1. ✅ The endpoint accepts join events from remote servers via PUT request
2. ✅ X-Matrix authentication is validated using `parse_x_matrix_auth()` and `validate_server_signature()`
3. ✅ Event structure is validated (type, sender, state_key, membership fields)
4. ✅ Event passes full PDU validation via `PduValidator` (6-step pipeline)
5. ✅ Resident server's signature is added via `sign_join_event()` helper
6. ✅ Event is persisted to database via `EventRepository::create()`
7. ✅ Membership record is created/updated via `MembershipRepository::create()`
8. ✅ Current room state is retrieved via `EventRepository::get_room_current_state()`
9. ✅ Complete auth chain is built via `EventRepository::get_auth_chain_for_events()`
10. ✅ Response is returned in v2 format: `{"state": [...], "auth_chain": [...], "event": {...}}`
11. ✅ Response includes the fully signed join event in the "event" field
12. ✅ The implementation matches v1 logic except for response format
13. ✅ Endpoint returns proper HTTP status codes (200 OK, 400 BAD_REQUEST, 401 UNAUTHORIZED, 403 FORBIDDEN, 404 NOT_FOUND, 500 INTERNAL_SERVER_ERROR)

## Implementation Complexity Assessment

**Complexity**: TRIVIAL

**Estimated Lines of Code**: ~390 lines (copied from v1)

**Lines to Change**: 4-10 lines (response format only)

**Risk Level**: MINIMAL - v1 implementation is proven and working

**Dependencies**: None - all required code exists

## Code Pattern Reference

For reference, here's the exact pattern used in v2 send_leave for changing response format:

**v1 send_leave response** (line 363):
```rust
let response = json!([
    200,
    {}
]);
```

**v2 send_leave response** (line 338):
```rust
let response = json!({});
```

Apply the same pattern to v2 send_join, but include state, auth_chain, and event fields.

## Notes on Event Propagation

**Important**: The current v1 implementation does NOT explicitly propagate events to other servers in the room. Event propagation may be handled by:
- Background services monitoring the database
- SurrealDB LIVE queries triggering federation sends
- Separate federation queue processors
- Or it may not be implemented yet

**Do NOT add event propagation logic** as part of this task. The v2 implementation should match v1 behavior exactly, except for response format.

## Cross-Reference: Similar v2 Migration

See v2 send_leave implementation for the exact pattern to follow:
- **v1**: [`packages/server/src/_matrix/federation/v1/send_leave/by_room_id/by_event_id.rs`](../packages/server/src/_matrix/federation/v1/send_leave/by_room_id/by_event_id.rs)
- **v2**: [`packages/server/src/_matrix/federation/v2/send_leave/by_room_id/by_event_id.rs`](../packages/server/src/_matrix/federation/v2/send_leave/by_room_id/by_event_id.rs)

Both implementations are nearly identical except for the response format change.

## Summary

This is a **copy-and-paste task** with a **single response format change**. All complex federation logic, validation, authorization, signing, and database operations are already fully implemented in the v1 send_join endpoint. The v2 endpoint needs identical logic with a different response format that includes the signed event.
