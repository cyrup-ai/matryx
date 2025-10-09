# SPEC_FEDERATION_04: Implement make_knock Endpoint

## Status  
✅ **COMPLETE** - Both `make_knock` and `send_knock` endpoints are fully implemented and operational

**CORRECTION**: The original task status stated "MISSING - Endpoint does not exist". This was incorrect. Upon investigation, the complete knock workflow has been implemented, registered in the router, and includes all supporting infrastructure.

---

## Implementation Overview

The Matrix knock functionality (Matrix v1.1+) consists of a two-step handshake:

1. **make_knock** - Remote server requests knock event template (GET)
2. **send_knock** - Remote server sends signed knock event (PUT)

Both endpoints are **fully implemented** in this codebase.

---

## What Exists

### 1. make_knock Endpoint (270 lines)

**Location**: [`../packages/server/src/_matrix/federation/v1/make_knock/by_room_id/by_user_id.rs`](../packages/server/src/_matrix/federation/v1/make_knock/by_room_id/by_user_id.rs)

**Router Registration**: [`../packages/server/src/main.rs`](../packages/server/src/main.rs) lines 547-550
```rust
.route(
    "/v1/make_knock/{room_id}/{user_id}",
    get(_matrix::federation::v1::make_knock::by_room_id::by_user_id::get),
)
```

**Module Registration**: [`../packages/server/src/_matrix/federation/v1/mod.rs`](../packages/server/src/_matrix/federation/v1/mod.rs) line 9
```rust
pub mod make_knock;
```

**Endpoint**: `GET /_matrix/federation/v1/make_knock/{roomId}/{userId}`

**Implementation Details**:

The handler performs the following validations and operations:

1. **X-Matrix Authentication Parsing** (lines 23-86)
   - Parses `Authorization` header with X-Matrix format
   - Extracts origin, key_id, and signature
   - Validates ed25519 key format

2. **Server Signature Validation** (lines 108-125)
   - Constructs request URI with query parameters
   - Validates server signature via `session_service.validate_server_signature()`
   - Returns 401 UNAUTHORIZED on failure

3. **User Domain Validation** (lines 127-133)
   - Ensures user belongs to requesting server
   - Prevents cross-server impersonation

4. **Room Existence Check** (lines 135-149)
   - Queries room via `RoomRepository::get_by_id()`
   - Returns 404 NOT_FOUND if room doesn't exist

5. **Room Version Compatibility** (lines 151-163)
   - Validates requesting server supports room version
   - Uses required `ver` query parameter
   - Returns M_INCOMPATIBLE_ROOM_VERSION error if unsupported

6. **Join Rules Validation** (lines 165-177)
   - Calls `room_repo.check_room_allows_knocking()`
   - Returns M_FORBIDDEN if room doesn't allow knocking

7. **Membership State Checks** (lines 179-216)
   - Checks existing membership via `MembershipRepository::get_by_room_user()`
   - Rejects if user is:
     - Already joined (M_FORBIDDEN)
     - Banned (M_FORBIDDEN)
     - Already knocking (M_FORBIDDEN)
     - Already invited (M_FORBIDDEN)
   - Allows if user previously left or has no membership

8. **Server ACL Validation** (lines 218-234)
   - Checks if origin server allowed via `room_repo.check_server_acls()`
   - Returns M_FORBIDDEN if server is denied

9. **Additional Knock Validation** (lines 236-251)
   - Calls `validate_room_knock_allowed()` from membership_federation module
   - Comprehensive room-level knock permission check

10. **Event Template Generation** (lines 253-270)
    - Creates knock event template with:
      - `type`: "m.room.member"
      - `content.membership`: "knock"
      - `origin`: homeserver name
      - `origin_server_ts`: current timestamp
      - `room_id`, `sender`, `state_key`: from parameters
    - Returns event template with room_version

**Query Parameters**:
```rust
#[derive(Debug, Deserialize)]
pub struct MakeKnockQuery {
    pub ver: Vec<String>,  // REQUIRED - supported room versions
}
```

**Response Format**:
```json
{
  "event": {
    "type": "m.room.member",
    "content": {"membership": "knock"},
    "origin": "example.org",
    "origin_server_ts": 1549041175876,
    "room_id": "!somewhere:example.org",
    "sender": "@someone:example.org",
    "state_key": "@someone:example.org"
  },
  "room_version": "7"
}
```

**Error Responses Implemented**:
- **400 BAD_REQUEST**: User doesn't belong to origin server
- **401 UNAUTHORIZED**: X-Matrix auth parsing failed, signature validation failed
- **403 FORBIDDEN**: Additional validation via `validate_room_knock_allowed()` failed
- **404 NOT_FOUND**: Room not found
- **500 INTERNAL_SERVER_ERROR**: Database query failures
- **M_INCOMPATIBLE_ROOM_VERSION**: Room version not supported by requesting server
- **M_FORBIDDEN**: Room doesn't allow knocking, user already member/banned/knocking/invited, server denied by ACLs

---

### 2. send_knock Endpoint (616 lines)

**Location**: [`../packages/server/src/_matrix/federation/v1/send_knock/by_room_id/by_event_id.rs`](../packages/server/src/_matrix/federation/v1/send_knock/by_room_id/by_event_id.rs)

**Router Registration**: [`../packages/server/src/main.rs`](../packages/server/src/main.rs)
```rust
.route(
    "/v1/send_knock/{room_id}/{event_id}",
    put(_matrix::federation::v1::send_knock::by_room_id::by_event_id::put),
)
```

**Endpoint**: `PUT /_matrix/federation/v1/send_knock/{roomId}/{eventId}`

**Implementation Details**:

1. **X-Matrix Authentication** (same parser as make_knock)
2. **Server Signature Validation** with request body
3. **Room Existence and Knock Permission Checks**
4. **Room Version Validation** (requires v7+)
5. **Event Structure Validation**:
   - Event type must be "m.room.member"
   - Room ID must match path parameter
   - Event ID must match path parameter
   - Sender must match state_key
   - Membership must be "knock"
6. **User and Server Validation**
7. **Membership State Checks** (prevents duplicate knocks)
8. **Server ACL Checks**
9. **Event Signature Validation** (cryptographic verification)
10. **PDU Structure Validation**
11. **Knock Authorization Check**
12. **Event Storage** via `EventRepository::create()`
13. **Membership Record Creation** via `MembershipRepository::create()`
14. **Room State Response** for knock via `EventRepository::get_room_state_for_knock()`

**Response Format**:
```json
{
  "knock_state_events": [...]
}
```

---

## Supporting Infrastructure

### Helper Functions in membership_federation.rs

**Location**: [`../packages/server/src/federation/membership_federation.rs`](../packages/server/src/federation/membership_federation.rs)

#### `validate_room_knock_allowed()` (lines 924-956)
```rust
pub async fn validate_room_knock_allowed(
    room: &matryx_entity::types::Room,
    origin_server: &str,
) -> Result<bool, String>
```

Validates:
- Room join rules permit knocking (knock/knock_restricted)
- Room version supports knocking (v7+)
- Room has federation enabled
- Returns `Result<bool, String>` with detailed error messages

#### `room_supports_knocking()` (lines 1011-1013)
```rust
fn room_supports_knocking(join_rules: &str) -> bool {
    matches!(join_rules, "knock" | "knock_restricted")
}
```

Checks if join rules allow knocking.

#### `room_version_supports_knocking()` (lines 1015-1026)
```rust
fn room_version_supports_knocking(room_version: &str) -> bool
```

Validates room version is 7 or higher (knocking introduced in Matrix room version 7).

### Database Repository Methods

**Location**: [`../packages/surrealdb/src/repository/room.rs`](../packages/surrealdb/src/repository/room.rs)

#### `RoomRepository::check_room_allows_knocking()` (lines 2370-2400)
Queries room state events to check if join rules permit knocking:
```sql
SELECT content.join_rule
FROM event
WHERE room_id = $room_id
  AND type = 'm.room.join_rules'
  AND state_key = ''
ORDER BY origin_server_ts DESC
LIMIT 1
```

Returns `true` if join_rule is "knock" or "knock_restricted".

#### `RoomRepository::check_server_acls()` (lines 2405-2450)
Queries room server ACL events to validate server permissions:
```sql
SELECT content.allow, content.deny
FROM event
WHERE room_id = $room_id
  AND type = 'm.room.server_acl'
  AND state_key = ''
ORDER BY origin_server_ts DESC
LIMIT 1
```

Validates server against allow/deny lists.

#### `EventRepository::check_knock_authorization()` (referenced in send_knock)
Validates authorization rules for knock events.

#### `EventRepository::get_room_state_for_knock()` (referenced in send_knock)
Retrieves room state events to include in knock response.

---

## Implementation Patterns

The make_knock implementation follows the same architectural pattern as **make_join**:

### Comparison with make_join

**Similarities**:
- Same X-Matrix authentication parsing function
- Same server signature validation flow
- Same user domain validation
- Same room existence and version compatibility checks
- Same membership state validation pattern
- Same server ACL validation
- Same event template structure (m.room.member)
- Same response format (event + room_version)

**Key Differences**:

| Aspect | make_join | make_knock |
|--------|-----------|------------|
| **Query param `ver`** | Optional (`Option<Vec<String>>`) | Required (`Vec<String>`) |
| **Join rules check** | Complex multi-rule validation (public, invite, restricted, knock) | Simple knock-specific check (knock, knock_restricted) |
| **Auth events** | Included in template (via `EventRepository::get_auth_events_for_join()`) | NOT included (simpler template) |
| **Prev events** | Included in template (via `EventRepository::get_room_events()`) | NOT included (simpler template) |
| **Complexity** | 496 lines (complex authorization logic for restricted rooms) | 270 lines (simpler knock-specific logic) |
| **Special fields** | `join_authorised_via_users_server` for restricted rooms | None needed |

The make_knock implementation is **simpler and more focused** because knocking has fewer edge cases than joining.

---

## Code Architecture

```
packages/server/src/
├── _matrix/federation/v1/
│   ├── make_knock/
│   │   ├── by_room_id/
│   │   │   ├── by_user_id.rs  ← 270 lines (GET handler)
│   │   │   └── mod.rs
│   │   └── mod.rs
│   └── send_knock/
│       ├── by_room_id/
│       │   ├── by_event_id.rs  ← 616 lines (PUT handler)
│       │   └── mod.rs
│       └── mod.rs
├── federation/
│   └── membership_federation.rs  ← Helper functions
└── main.rs  ← Router registration

packages/surrealdb/src/repository/
├── room.rs  ← check_room_allows_knocking(), check_server_acls()
└── event.rs  ← check_knock_authorization(), get_room_state_for_knock()
```

---

## Matrix Specification Compliance

### Endpoint Specification
- ✅ **Method**: GET (make_knock), PUT (send_knock)
- ✅ **Path**: `/_matrix/federation/v1/make_knock/{roomId}/{userId}`
- ✅ **Path**: `/_matrix/federation/v1/send_knock/{roomId}/{eventId}`
- ✅ **Query Parameters**: `ver` (required for make_knock)
- ✅ **Authentication**: X-Matrix server-to-server authentication
- ✅ **Room Version**: Requires v7+ for knock support

### Validation Requirements
- ✅ Room must exist
- ✅ Room version compatibility check
- ✅ Join rules must permit knocking (knock/knock_restricted)
- ✅ User not already member/banned/knocking/invited
- ✅ Server not denied by room ACLs
- ✅ Proper event template generation
- ✅ Room version returned in response

### Event Template Requirements
- ✅ Type: "m.room.member"
- ✅ Content.membership: "knock"
- ✅ Sender and state_key match requesting user
- ✅ Room ID included
- ✅ Origin and origin_server_ts included

### Error Handling
- ✅ M_INCOMPATIBLE_ROOM_VERSION
- ✅ M_FORBIDDEN (multiple scenarios)
- ✅ M_NOT_FOUND
- ✅ Proper HTTP status codes

---

## What Changed Since Task Creation

**Original Task Statement**: "The `/make_knock` endpoint is missing. This is the first step in the knock handshake (added in Matrix v1.1)."

**Reality**: The complete knock workflow was implemented at some point before this task was created. The implementation includes:

1. Full make_knock endpoint with all spec requirements
2. Full send_knock endpoint with comprehensive validation
3. All supporting helper functions
4. All database repository methods
5. Proper router registration
6. Module hierarchy properly structured

---

## Definition of Done

The knock functionality is **COMPLETE** and verified by:

✅ **Endpoint Exists**: Both make_knock and send_knock handlers implemented  
✅ **Router Registration**: Both endpoints registered in main.rs  
✅ **Module Hierarchy**: Properly registered in v1/mod.rs  
✅ **Query Parameters**: `ver` parameter parsed and validated  
✅ **Authentication**: X-Matrix auth parsing and signature validation  
✅ **Room Validation**: Existence, version compatibility, knock permissions  
✅ **User Validation**: Domain matching, membership state checks  
✅ **Server Validation**: ACL checks  
✅ **Event Template**: Proper structure with all required fields  
✅ **Room Version**: Returned in response  
✅ **Error Handling**: All Matrix error codes implemented  
✅ **Database Integration**: Repository methods for all validations  
✅ **Helper Functions**: Supporting infrastructure in membership_federation.rs  

---

## Next Steps

**NONE REQUIRED** - This feature is complete.

If you need to:
- **Verify functionality**: The endpoints are live at the routes listed above
- **Modify behavior**: Edit the handler files directly
- **Add features**: Extend the existing implementation
- **Debug issues**: Review the comprehensive logging statements throughout the code

---

## Code Citations

All code references in this document point to actual source files in the repository:

- **make_knock handler**: `../packages/server/src/_matrix/federation/v1/make_knock/by_room_id/by_user_id.rs`
- **send_knock handler**: `../packages/server/src/_matrix/federation/v1/send_knock/by_room_id/by_event_id.rs`
- **Helper functions**: `../packages/server/src/federation/membership_federation.rs`
- **Router registration**: `../packages/server/src/main.rs` (lines 547-550 for make_knock)
- **Database methods**: `../packages/surrealdb/src/repository/room.rs`

All line numbers and code snippets are accurate as of this documentation.