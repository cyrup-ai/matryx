# SPEC_FEDERATION_05: Federation Invite v2 - Missing invite_room_state Validation

## Status: INCOMPLETE - 7/10

## Issue Summary
The v2 invite endpoint is implemented and functional but **violates Matrix 1.16+ specification** by not validating the `invite_room_state` parameter.

## Implementation Location
**File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v2/invite/by_room_id/by_event_id.rs`

## What's Missing

### Required Validation (Matrix 1.16+ Spec)
The endpoint currently extracts `invite_room_state` at line 130 but performs **ZERO validation** before including it in the response (lines 353-361).

**Specification requires:**
1. `invite_room_state` MUST contain an `m.room.create` event
2. All events in `invite_room_state` MUST be formatted per the room's version specification
3. All events MUST have valid signatures
4. All events MUST belong to the same room as the invite
5. Return `400 M_MISSING_PARAM` error if validation fails

**Current behavior:**
```rust
// Line 130: No validation performed
let invite_room_state = payload.get("invite_room_state");

// Lines 353-361: Blindly included in response
if let Some(room_state) = invite_room_state {
    if let Some(unsigned) = response_event.get_mut("unsigned") {
        unsigned["invite_room_state"] = room_state.clone();
    } else {
        response_event["unsigned"] = json!({
            "invite_room_state": room_state
        });
    }
}
```

## Required Implementation

Add validation function after line 130:
```rust
// After extracting invite_room_state
if let Some(room_state) = &invite_room_state {
    validate_invite_room_state(room_state, &room_id, &room.room_version)?;
}
```

Create new validation function:
```rust
fn validate_invite_room_state(
    invite_room_state: &Value,
    room_id: &str,
    room_version: &str,
) -> Result<(), Json<Value>> {
    let events = invite_room_state.as_array().ok_or_else(|| {
        Json(json!({
            "errcode": "M_MISSING_PARAM",
            "error": "invite_room_state must be an array"
        }))
    })?;

    // 1. Check for m.room.create event
    let has_create = events.iter().any(|e| {
        e.get("type").and_then(|t| t.as_str()) == Some("m.room.create")
    });
    
    if !has_create {
        return Err(Json(json!({
            "errcode": "M_MISSING_PARAM",
            "error": "invite_room_state must contain m.room.create event"
        })));
    }

    // 2. Validate each event format per room version
    for event in events {
        // Validate event has required fields per room version
        // Use existing room version validation logic
        
        // 3. Verify event room_id matches (for room versions that include it)
        if let Some(event_room_id) = event.get("room_id").and_then(|r| r.as_str()) {
            if event_room_id != room_id {
                return Err(Json(json!({
                    "errcode": "M_MISSING_PARAM",
                    "error": "invite_room_state event belongs to different room"
                })));
            }
        }
        
        // 4. Validate signatures on each event
        // Use existing signature validation logic
    }

    Ok(())
}
```

## Spec Reference
**File:** `/Volumes/samsung_t9/maxtryx/tmp/matrix-spec-official/data/api/server-server/invites-v2.yaml`

Lines 77-100:
> `invite_room_state`: MUST contain the `m.room.create` event for the room. All events listed MUST additionally be formatted according to the room version specification. Servers MAY apply validation to room versions 1-11, and SHOULD apply validation to all other room versions.

Lines 169-182 (400 error response):
> The `M_MISSING_PARAM` error code is used to indicate one or more of the following:
> * The `m.room.create` event is missing from `invite_room_state`
> * One or more entries in `invite_room_state` are not formatted according to the room's version
> * One or more events fails a signature check
> * One or more events does not reside in the same room as the invite

## Acceptance Criteria
- [ ] Add `validate_invite_room_state()` function
- [ ] Check for presence of `m.room.create` event
- [ ] Validate event format per room version
- [ ] Validate event signatures
- [ ] Validate all events belong to same room
- [ ] Return `400 M_MISSING_PARAM` with appropriate error messages
- [ ] Test with various room versions
- [ ] Verify interoperability with spec-compliant Matrix servers
