# SPEC_FEDERATION_01: Complete v2 send_join Implementation

## Status
INCOMPLETE - Currently a stub

## Description
The v2 send_join endpoint is currently a stub returning hardcoded JSON. The spec requires a full implementation that properly processes join events.

## Spec Requirements (spec/server/09-room-joins.md)

### Endpoint
`PUT /_matrix/federation/v2/send_join/{roomId}/{eventId}`

### What's Required
1. Validate and sign the join event
2. Add resident server's signature to the event
3. Accept the event into the room's graph
4. Return full room state and auth chain
5. Propagate the event to other servers in the room

### Response Format
```json
{
  "auth_chain": [<PDUs>],
  "state": [<PDUs>],
  "event": <signed join event>
}
```

### Key Differences from v1
- Response is a proper JSON object (not array format like v1)
- More standardized and preferred over v1
- Fallback to v1 if server returns 400/404

## Current Implementation
**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v2/send_join/by_room_id/by_event_id.rs`

Current code is a stub:
- Returns hardcoded empty arrays
- No event validation
- No signature addition
- No state retrieval
- No event propagation

## What Needs to be Done

1. **Parse and validate join event from request body**
   - Validate event structure per room version
   - Check auth_events are present
   - Verify signatures

2. **Authorization checks**
   - Verify user can join room
   - Check join rules (public/invite/restricted)
   - Validate restricted room conditions if applicable
   - Add `join_authorised_via_users_server` signature

3. **Add server signature**
   - Sign the event with resident server's key
   - Add signature to event.signatures

4. **Retrieve room state**
   - Get full room state before the join
   - Build complete auth chain
   - Format PDUs according to room version

5. **Persist and propagate event**
   - Store event in database
   - Update room membership
   - Send to other servers via federation

6. **Return proper response**
   - Include signed event
   - Include state array
   - Include auth_chain array

## Files to Reference
- v1 implementation: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs`
- Auth logic: `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/authorization.rs`
- Event signing: `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/event_signer.rs`

## Verification
- [ ] Event is properly validated
- [ ] Resident server signature is added
- [ ] Room state is returned correctly
- [ ] Auth chain is complete
- [ ] Event is persisted to database
- [ ] Event is propagated to other servers
- [ ] Response matches spec format
- [ ] Works with different room versions

## Priority
HIGH - This is a core federation endpoint for room joins
