# SPEC_FEDERATION_09: Complete get_missing_events Implementation

## Status
INCOMPLETE - Needs proper BFS algorithm

## Description
The get_missing_events endpoint exists but may need validation against spec requirements for proper breadth-first traversal.

## Spec Requirements (spec/server/22-backfill-events.md)

### Endpoint
`POST /_matrix/federation/v1/get_missing_events/{roomId}`

### What's Required
1. Breadth-first walk of prev_events
2. Start from latest_events
3. Stop at earliest_events or limit
4. Respect min_depth parameter

### Request Body
```json
{
  "earliest_events": ["$missing_event:example.org"],
  "latest_events": ["$event_with_missing_prev:example.org"],
  "limit": 10,
  "min_depth": 0
}
```

### Response Format (200)
```json
{
  "events": [
    {
      "content": {"see_room_version_spec": "..."},
      "room_id": "!somewhere:example.org",
      "type": "m.room.minimal_pdu"
    }
  ]
}
```

## What Needs to be Done

1. **Verify BFS implementation**
   - Start from latest_events
   - Walk backward via prev_events
   - Stop at earliest_events
   - Respect depth limit

2. **Check limit enforcement**
   - Default to 10 if not provided
   - Never return more than limit

3. **Verify min_depth handling**
   - Skip events below min_depth
   - Default to 0

4. **Authorization checks**
   - Verify requesting server in room
   - Check event visibility

5. **Response formatting**
   - Return events array
   - Format per room version

## Current Implementation
**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/get_missing_events/by_room_id.rs`

Need to verify:
- [ ] Proper BFS traversal
- [ ] earliest_events exclusion
- [ ] limit enforcement
- [ ] min_depth handling
- [ ] Room version compatibility

## Verification
- [ ] BFS algorithm correct
- [ ] earliest_events skipped
- [ ] limit respected
- [ ] min_depth applied
- [ ] Server authorized
- [ ] Events formatted correctly

## Priority
MEDIUM - Important for event graph consistency
