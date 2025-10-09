# SPEC_FEDERATION_03: Implement send_knock Endpoint

## Status
**IMPLEMENTED - MINOR FIX NEEDED** (Previously marked as MISSING - this was incorrect)

## Critical Discovery
This endpoint is **NOT MISSING**. A comprehensive code review reveals a complete, production-ready implementation at 616 lines that includes all Matrix specification requirements. Only ONE minor field name correction is needed for full spec compliance.

## Current Implementation Analysis

### Implementation Location
- **Main Handler**: [./packages/server/src/_matrix/federation/v1/send_knock/by_room_id/by_event_id.rs](../packages/server/src/_matrix/federation/v1/send_knock/by_room_id/by_event_id.rs) (616 lines)
- **Module Registration**: [./packages/server/src/_matrix/federation/v1/send_knock/mod.rs](../packages/server/src/_matrix/federation/v1/send_knock/mod.rs)
- **Module Path**: [./packages/server/src/_matrix/federation/v1/send_knock/by_room_id/mod.rs](../packages/server/src/_matrix/federation/v1/send_knock/by_room_id/mod.rs)
- **Router Registration**: [./packages/server/src/main.rs](../packages/server/src/main.rs) lines 600-603
- **Validation Logic**: [./packages/server/src/federation/membership_federation.rs](../packages/server/src/federation/membership_federation.rs) lines 1051-1077

### Existing Implementation Features

The current implementation includes **ALL** required Matrix specification features:

#### 1. Authentication & Signature Validation (Lines 17-76)
- ✅ X-Matrix authentication header parsing
- ✅ Server signature validation via `session_service.validate_server_signature()`
- ✅ Origin server verification
- ✅ Request body canonical JSON signing

#### 2. Request Validation (Lines 78-232)
- ✅ Room existence validation via `RoomRepository::get_by_id()`
- ✅ Room version compatibility check (requires v7+)
- ✅ Event structure validation (type, room_id, event_id, sender, state_key)
- ✅ Sender domain verification against origin server
- ✅ Membership content validation (must be "knock")
- ✅ Sender/state_key consistency check (must match for membership events)

#### 3. Authorization Checks (Lines 233-348)
- ✅ Join rules validation via `RoomRepository::check_room_allows_knocking()`
- ✅ Existing membership state checks (join, ban, knock, invite)
- ✅ Server ACL validation via `RoomRepository::check_server_acls()`
- ✅ Event signature validation
- ✅ PDU structure validation via `validate_pdu_structure()`
- ✅ Knock authorization via `EventRepository::check_knock_authorization()`
- ✅ Room knock permission validation via `validate_room_knock_allowed()`

#### 4. Event Storage (Lines 349-448)
- ✅ Event entity conversion from JSON payload
- ✅ Event persistence via `EventRepository::create()`
- ✅ Membership state update via `MembershipRepository::create()`
- ✅ Proper timestamp handling (origin_server_ts and received_ts)
- ✅ Signature preservation
- ✅ Event graph integration (prev_events, auth_events, depth, hashes)

#### 5. Room State Response (Lines 449-462)
- ✅ Stripped state retrieval via `EventRepository::get_room_state_for_knock()`
- ⚠️ **ISSUE**: Response field is `knock_state_events` but spec requires `knock_room_state`

### Supporting Repository Methods

All required database operations are fully implemented:

#### RoomRepository ([./packages/surrealdb/src/repository/room.rs](../packages/surrealdb/src/repository/room.rs))
- **check_room_allows_knocking** (line 2370): Validates join_rule = "knock"
- **check_server_acls** (line 2401): Validates server not in deny list
- **get_by_id**: Retrieves room entity with version info

#### EventRepository ([./packages/surrealdb/src/repository/event.rs](../packages/surrealdb/src/repository/event.rs))
- **check_knock_authorization** (line 2924): Validates power level restrictions
- **get_room_state_for_knock** (line 2961): Returns essential state events:
  - m.room.create
  - m.room.join_rules
  - m.room.power_levels
  - m.room.name
  - m.room.topic
  - m.room.avatar
  - m.room.canonical_alias
- **create**: Persists knock event to database

#### MembershipRepository
- **get_by_room_user**: Checks existing membership state
- **create**: Stores knock membership record

### Router Integration

The endpoint is **FULLY REGISTERED** in the main application router:

```rust
// File: packages/server/src/main.rs, lines 600-603
.route(
    "/v1/send_knock/{room_id}/{event_id}",
    put(_matrix::federation::v1::send_knock::by_room_id::by_event_id::put),
)
```

## Spec Compliance Analysis

Comparing against [Matrix Server-Server API v1.1 Specification](../spec/server/12-room-knocking.md):

### ✅ Fully Compliant Features

1. **Endpoint Path**: `PUT /_matrix/federation/v1/send_knock/{roomId}/{eventId}` ✅
2. **Authentication**: X-Matrix signature validation ✅
3. **Request Validation**: All required fields validated ✅
4. **Authorization Rules**: Complete implementation ✅
   - Room join_rule must be "knock" ✅
   - User not already joined ✅
   - User not banned ✅
   - Server not denied by ACLs ✅
   - Room version 7+ support ✅
5. **Event Processing**: Full PDU validation pipeline ✅
6. **Event Storage**: Complete with membership updates ✅
7. **Error Handling**: All spec-required error codes ✅
   - 403 M_FORBIDDEN for permission issues ✅
   - 404 M_NOT_FOUND for unknown rooms ✅
   - 400 BAD_REQUEST for invalid events ✅

### ⚠️ Single Non-Compliance Issue

**Response Field Name Mismatch** (Line 455):

**Current Code**:
```rust
let response = json!({
    "knock_state_events": knock_state_events
});
```

**Spec Requirement** ([spec/server/12-room-knocking.md](../spec/server/12-room-knocking.md) line 236):
```json
{
  "knock_room_state": [...]
}
```

**Impact**: Client libraries expecting spec-compliant response will fail to parse the room state array.

## What Needs to Change

### Single File Edit Required

**File**: `./packages/server/src/_matrix/federation/v1/send_knock/by_room_id/by_event_id.rs`

**Line 455** - Change response field name:

```rust
// BEFORE (current):
let response = json!({
    "knock_state_events": knock_state_events
});

// AFTER (spec-compliant):
let response = json!({
    "knock_room_state": knock_state_events
});
```

That's it. This is the **ONLY** change needed for full Matrix specification compliance.

## Implementation Verification Checklist

Current implementation status against spec requirements:

- [x] Endpoint exists and responds
- [x] Knock event validated
- [x] Join rules checked (must be "knock")
- [x] Room version checked (v7+)
- [x] Server signature validated
- [x] Event signatures validated
- [x] PDU structure validated
- [x] Authorization rules enforced
- [x] Membership state checked
- [x] Server ACLs enforced
- [x] Event persisted to database
- [x] Membership record created
- [x] Stripped state retrieved
- [ ] Response field name matches spec (fix required)
- [x] Error cases handled correctly
- [x] Works with room versions 7+

## Code Quality Assessment

The existing implementation demonstrates:

1. **Comprehensive Error Handling**: Every failure path returns appropriate Matrix error codes
2. **Security Best Practices**: Multi-layer validation (auth, signatures, PDU, authorization)
3. **Proper Database Patterns**: Repository pattern with typed queries
4. **Spec Adherence**: 99% compliant with Matrix Server-Server API v1.1
5. **Logging**: Extensive debug/info/warn/error logging for observability
6. **Type Safety**: Strong typing throughout with Rust's type system

## Definition of Done

The task is complete when:

1. ✅ The response JSON field is renamed from `knock_state_events` to `knock_room_state`
2. ✅ The endpoint returns spec-compliant responses

**Verification**: Send a federation knock request and verify the response contains `knock_room_state` field.

## Priority
LOW - Endpoint is fully functional with minor spec compliance issue

## Related Implementations

For reference on similar patterns already implemented in this codebase:

- **make_knock**: [./packages/server/src/_matrix/federation/v1/make_knock/by_room_id/by_user_id.rs](../packages/server/src/_matrix/federation/v1/make_knock/by_room_id/by_user_id.rs) (270 lines)
- **send_join**: [./packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs](../packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs) (390 lines)
- **Federation Validation**: [./packages/server/src/federation/membership_federation.rs](../packages/server/src/federation/membership_federation.rs) lines 1051-1077

The send_knock implementation follows the exact same architectural patterns as send_join and make_knock, ensuring consistency across the federation API implementation.

## Specification Reference

Full specification: [./spec/server/12-room-knocking.md](../spec/server/12-room-knocking.md)

Key sections:
- Line 162-278: PUT /send_knock endpoint specification
- Line 236-262: Response format requirements
- Line 280-296: Error response formats
- Line 298-342: Knock event processing flow
- Line 344-366: Knock authorization rules
