# SPEC_FEDERATION_03: Fix send_knock Response Field Name

## Status
**ONE LINE FIX NEEDED** - Spec compliance issue

## Issue

The `send_knock` endpoint implementation is otherwise complete and production-ready, but has ONE critical spec compliance bug:

**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/send_knock/by_room_id/by_event_id.rs`

**Lines 432-435** (current):
```rust
let response = json!({
    "knock_state_events": knock_state_events
});
```

**Required fix** (per Matrix Server-Server API spec):
```rust
let response = json!({
    "knock_room_state": knock_state_events
});
```

## Spec Reference

Matrix Server-Server API v1.1, PUT /_matrix/federation/v1/send_knock/{roomId}/{eventId}

The 200 response MUST contain:
- Field name: `knock_room_state` (not `knock_state_events`)
- Type: `[StrippedStateEvent]`
- Required: Yes

**Specification**: `/Volumes/samsung_t9/maxtryx/spec/server/12-room-knocking.md` lines 234-236

## Impact

- **Severity**: CRITICAL for spec compliance
- **Interoperability**: Breaks compatibility with Matrix clients/servers expecting spec-compliant responses
- **Effort**: Trivial - one word change on one line

## Verification

After fix, verify the response contains `knock_room_state` field instead of `knock_state_events`.

## Priority
HIGH - Spec compliance violation
