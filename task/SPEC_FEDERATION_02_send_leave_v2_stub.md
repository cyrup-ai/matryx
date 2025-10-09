# SPEC_FEDERATION_02: Complete v2 send_leave Implementation

## Status
INCOMPLETE - Currently a stub

## Description
The v2 send_leave endpoint is currently a stub. The spec requires proper leave event processing.

## Spec Requirements (spec/server/10-room-leaves.md)

### Endpoint
`PUT /_matrix/federation/v2/send_leave/{roomId}/{eventId}`

### What's Required
1. Validate the leave event
2. Sign the event with resident server's signature
3. Accept into room graph
4. Return empty JSON object (not array like v1)
5. Propagate to other servers

### Response Format
```json
{}
```

### Key Differences from v1
- Response is `{}` not `[200, {}]`
- Preferred over v1 API
- Fallback to v1 if 400/404

## Current Implementation
**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v2/send_leave/by_room_id/by_event_id.rs`

Current code is a stub returning hardcoded response.

## What Needs to be Done

1. **Parse and validate leave event**
   - Validate event structure
   - Check membership is "leave"
   - Verify user is in room

2. **Authorization checks**
   - User must be in room to leave
   - Validate event signatures
   - Check not banned (banned users can't leave)

3. **Add server signature**
   - Sign event with resident server key

4. **Persist and propagate**
   - Store in database
   - Update membership state
   - Send to other servers

5. **Return proper response**
   - Return empty JSON object `{}`

## Files to Reference
- v1 implementation: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/send_leave/by_room_id/by_event_id.rs`

## Verification
- [ ] Leave event validated
- [ ] Signature added
- [ ] Event persisted
- [ ] Membership updated
- [ ] Event propagated
- [ ] Returns `{}` not `[200, {}]`

## Priority
MEDIUM - Important for clean leave handling
