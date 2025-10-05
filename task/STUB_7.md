# STUB_7: Room Directory Authorization

## OBJECTIVE

Implement power level authorization checks for room directory visibility changes. Currently, any authenticated user can modify a room's directory visibility, violating the Matrix specification requirement that only room admins/moderators should have this permission.

## SEVERITY

**CRITICAL SECURITY ISSUE**

## LOCATION

- **Primary File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/directory/list/room/by_room_id.rs:66`

## CURRENT STUB CODE

```rust
// According to Matrix spec, only room admins/moderators should be able to change directory visibility
// This requires checking user's power level in the room
// For now, we log the user_id for audit purposes
tracing::info!(
    "User {} requesting to change room {} directory visibility to {}",
```

## SUBTASKS

### SUBTASK1: Understand Matrix Power Levels

**What:** Research Matrix room power level requirements  
**Where:** Matrix Client-Server specification  
**Why:** Need to understand authorization model  

**Requirements:**
- Download Matrix spec on power levels and room state
- Save to `/Volumes/samsung_t9/maxtryx/docs/matrix-power-levels.md`
- Document default power levels for actions
- Document what power level is required for directory visibility changes
- Understand how power levels are stored (m.room.power_levels state event)

### SUBTASK2: Locate Power Level Repository Methods

**What:** Find or create methods to check user power levels  
**Where:** `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/`  
**Why:** Need to query user's power in room  

**Requirements:**
- Look for existing power level checking methods
- If exists, document the method signature
- If not exists, plan to implement:
  - `get_user_power_level(room_id, user_id) -> Result<i64>`
  - `check_user_can_perform_action(room_id, user_id, required_level) -> Result<bool>`
- Understand how m.room.power_levels state is stored

### SUBTASK3: Determine Required Power Level

**What:** Define what power level is needed for directory visibility  
**Where:** Research or define in configuration  
**Why:** Need threshold for authorization check  

**Requirements:**
- Check Matrix spec for defined requirement
- If not specified in spec, determine appropriate level:
  - Default users: 0
  - Moderators: 50
  - Admins: 100
- Document decision
- Consider making it configurable via power_levels event

### SUBTASK4: Implement Authorization Check

**What:** Add power level check before allowing directory change  
**Where:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/directory/list/room/by_room_id.rs:66`  
**Why:** Enforce authorization  

**Requirements:**
- Remove stub comment
- Call power level checking method
- Determine required level (likely 50 for moderator)
- If user lacks permission, return Matrix error:
  - Error code: `M_FORBIDDEN`
  - Message: "You don't have permission to change room directory visibility"
- Keep audit logging
- Only proceed with change if authorized

### SUBTASK5: Handle Edge Cases

**What:** Address special cases in authorization  
**Where:** Same file as SUBTASK4  
**Why:** Complete implementation  

**Requirements:**
- Handle room creator (always allowed)
- Handle missing power_levels event (use defaults)
- Handle user not in room (should already be rejected)
- Handle local vs federated users
- Document behavior for each case

## DEFINITION OF DONE

- [ ] Power level requirements documented from Matrix spec
- [ ] Power level checking method identified or implemented
- [ ] Authorization check added before directory changes
- [ ] Appropriate error returned when unauthorized
- [ ] Edge cases handled
- [ ] Stub comment removed
- [ ] Audit logging maintained
- [ ] Code compiles without errors

## RESEARCH NOTES

### Matrix Power Levels Structure

**m.room.power_levels event:**
```json
{
  "type": "m.room.power_levels",
  "content": {
    "users": {
      "@user:example.com": 100
    },
    "users_default": 0,
    "events": {
      "m.room.name": 50
    },
    "events_default": 0,
    "state_default": 50
  }
}
```

### Matrix Error Format

**M_FORBIDDEN response:**
```json
{
  "errcode": "M_FORBIDDEN",
  "error": "You don't have permission to..."
}
```

### Related Code

**Look for:**
- Existing power level checks in other endpoints
- Room state event querying
- Similar authorization patterns
- Error response helpers

## SECURITY IMPLICATIONS

**Current vulnerability:**
- Any room member can change directory visibility
- Could expose private rooms unintentionally
- Could hide public rooms maliciously

**After fix:**
- Only authorized users can modify visibility
- Matches Matrix specification
- Prevents unauthorized disclosure

## NO TESTS OR BENCHMARKS

Do NOT write unit tests, integration tests, or benchmarks as part of this task. The testing team will handle test coverage separately.

---

## MATRIX SPECIFICATION REQUIREMENTS

### Power Levels and Room Directory Authorization

From `/spec/server/09-room-joins.md` and `/spec/server/11-room-invites.md`:

**Power Level Requirements:**

> User's power level must meet the required join level (default: 0)
> For restricted rooms, the authorizing server user must have invite permissions

**Power Levels Event Structure:**

The `m.room.power_levels` state event defines authorization:

```json
{
  "type": "m.room.power_levels",
  "content": {
    "users": {
      "@user:example.com": 100
    },
    "users_default": 0,
    "events": {
      "m.room.name": 50,
      "m.room.power_levels": 100
    },
    "events_default": 0,
    "state_default": 50,
    "ban": 50,
    "kick": 50,
    "redact": 50,
    "invite": 0
  }
}
```

**Authorization Rules:**

From the specification:

1. **Power Levels**: The sender must have sufficient power level to perform actions
2. **State Events**: Default requirement is `state_default` (typically 50)
3. **Custom Actions**: Can define specific power requirements per event type

**Room Directory Visibility:**

While not explicitly specified in the provided specs, room directory visibility changes are typically state-modifying operations that require:

- Moderator level (50) or higher
- OR explicit permission in power_levels.events for directory changes
- Room creator always has permission

**Authorization Flow:**

1. Extract user_id from authenticated session
2. Query m.room.power_levels state event for the room
3. Determine user's power level:
   - Check `users[user_id]` field
   - If not present, use `users_default` (typically 0)
4. Compare against required level for action
5. Reject with `M_FORBIDDEN` if insufficient

**Auth Events Requirements:**

From `/spec/server/09-room-joins.md`:

> Join events must include appropriate auth events:
> - m.room.create event
> - m.room.power_levels event (if present)
> - Sender's current m.room.member event (if present)

**Error Codes:**

- `M_FORBIDDEN` - User lacks required power level
- Recommended message: "You don't have permission to change room directory visibility"

**Default Power Levels:**

- Room creator: 100
- Default user: 0
- State changes: 50 (state_default)
- Moderator actions (ban, kick): 50
