# Federation API Spec Compliance - Remaining Issues

## QA Review Summary (2025-10-09)

**Rating: 9/10** - Near complete implementation with only 3 stub endpoints remaining

### Review Findings

Out of 12 originally identified gaps, **9 are FULLY IMPLEMENTED** with production-quality code:
- ✅ send_knock endpoint (fully implemented)
- ✅ make_knock endpoint (fully implemented)
- ✅ invite v2 endpoint (fully implemented)
- ✅ send_leave v2 endpoint (fully implemented)
- ✅ public_rooms endpoint (fully implemented)
- ✅ PDU validation pipeline (complete 6-step implementation)
- ✅ EDU processing (all types: typing, receipt, presence, device_list, signing_key, direct_to_device)
- ✅ Server key query (complete with caching and verification)
- ✅ get_missing_events BFS algorithm (proper breadth-first traversal)

### Outstanding Issues: 3 Stub Implementations

## 1. CRITICAL: v2 send_join Stub Implementation

**File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v2/send_join/by_room_id/by_event_id.rs`

**Current State:** Returns hardcoded JSON response without validation

```rust
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

**Required Implementation:**

The v2 send_join endpoint MUST:
1. Parse X-Matrix authentication header
2. Validate server signature
3. Extract and validate join event from payload
4. Run full PDU validation pipeline (use existing PduValidator)
5. Validate user belongs to requesting server
6. Check room membership state
7. Store validated join event in database
8. Update membership record to "join" state
9. Add server signature to the event
10. Return v2 response format with:
    - `state`: Current room state events
    - `auth_chain`: Authorization chain for the join
    - `event`: The signed join event (NOT inside `room_state`)
    - `members_omitted`: Boolean flag (optional)
    - `servers_in_room`: List of participating servers (optional)

**Key Differences from v1:**
- v1 returns: `[200, {...}]` (tuple format)
- v2 returns: `{...}` (direct object format)
- v2 includes additional metadata fields

**Reference Implementation Pattern:**
See `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v2/send_leave/by_room_id/by_event_id.rs` for complete pattern including:
- Proper authentication
- PDU validation using PduValidator
- Database operations
- Signature addition
- Correct v2 response format

---

## 2. HIGH: Query Directory Stub

**File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/query/directory.rs`

**Current State:** Returns empty JSON `{}`

```rust
pub async fn get() -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
```

**Required Implementation:**

GET `/_matrix/federation/v1/query/directory?room_alias={roomAlias}`

Must implement room alias resolution:
1. Parse X-Matrix authentication
2. Validate server signature
3. Extract `room_alias` query parameter
4. Validate alias format (`#alias:server.com`)
5. Check if alias is local to this server
6. Query room_aliases table for the alias
7. Return room ID and participating servers

**Response Format:**
```json
{
  "room_id": "!roomid:server.com",
  "servers": ["server.com", "other-server.com"]
}
```

**Error Cases:**
- `M_NOT_FOUND`: Alias not found
- `M_INVALID_PARAM`: Invalid alias format

**Database Query:**
```sql
SELECT room_id, servers FROM room_aliases WHERE alias = $alias
```

---

## 3. MEDIUM: 3PID onbind Stub

**File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/threepid/onbind.rs`

**Current State:** Returns empty JSON `{}`

```rust
pub async fn put(Json(_payload): Json<Value>) -> Result<Json<Value>, StatusCode> {
    Ok(Json(json!({})))
}
```

**Required Implementation:**

PUT `/_matrix/federation/v1/3pid/onbind`

Notifies server when a third-party identifier is bound to a user on an identity server.

**Request Payload:**
```json
{
  "invites": [{
    "medium": "email",
    "address": "user@example.com",
    "mxid": "@user:server.com",
    "room_id": "!room:server.com",
    "sender": "@inviter:server.com",
    "signed": {
      "mxid": "@user:server.com",
      "token": "random_token",
      "signatures": {...}
    }
  }]
}
```

**Implementation Steps:**
1. Parse X-Matrix authentication
2. Validate server signature
3. Extract invites array from payload
4. For each invite:
   - Validate mxid belongs to this server
   - Verify signed object signatures
   - Check room_id exists
   - Create pending third-party invite in database
   - Send invite event to room
5. Return empty object `{}`

**Database Operations:**
- Store third_party_invites with pending status
- Create m.room.third_party_invite event
- Link invite to user when they register with matching 3PID

**Validation:**
- MUST verify signatures on signed object
- MUST check mxid belongs to local server
- MUST validate room exists
- SHOULD rate-limit invites per IP/server

---

## Testing Requirements

For each implementation:
1. Unit tests for validation logic
2. Integration tests with mock federation server
3. Signature verification tests
4. Error handling for all edge cases
5. Database transaction rollback on errors

## Matrix Specification References

- **v2 send_join**: https://spec.matrix.org/v1.11/server-server-api/#put_matrixfederationv2send_joinroomideventid
- **Query Directory**: https://spec.matrix.org/v1.11/server-server-api/#get_matrixfederationv1querydirectory
- **3PID onbind**: https://spec.matrix.org/v1.11/server-server-api/#put_matrixfederationv13pidonbind

---

**Review Date:** 2025-10-09
**Reviewer:** Claude (Sonnet 4.5)
**Previous Issues Resolved:** 9/12 (75% completion)
**Remaining Work:** 3 stub implementations (25%)
