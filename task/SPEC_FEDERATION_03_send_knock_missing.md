# SPEC_FEDERATION_03: Implement send_knock Endpoint

## Status
MISSING - Endpoint does not exist

## Description
The `/send_knock` endpoint is completely missing from the implementation. This is required for room knocking functionality (added in Matrix v1.1).

## Spec Requirements (spec/server/12-room-knocking.md)

### Endpoint
`PUT /_matrix/federation/v1/send_knock/{roomId}/{eventId}`

### What's Required
1. Accept signed knock event from knocking server
2. Validate knock event structure
3. Check room allows knocking (join_rule = "knock")
4. Accept event into room graph
5. Return stripped room state to help identify the room

### Request Body
```json
{
  "content": {
    "membership": "knock"
  },
  "origin": "example.org",
  "origin_server_ts": 1549041175876,
  "sender": "@someone:example.org",
  "state_key": "@someone:example.org",
  "type": "m.room.member"
}
```

### Response Format (200)
```json
{
  "knock_room_state": [
    {
      "content": {"name": "Example Room"},
      "sender": "@bob:example.org",
      "state_key": "",
      "type": "m.room.name"
    },
    {
      "content": {"join_rule": "knock"},
      "sender": "@bob:example.org",
      "state_key": "",
      "type": "m.room.join_rules"
    }
  ]
}
```

## What Needs to be Done

1. **Create endpoint file**
   - Path: `/packages/server/src/_matrix/federation/v1/send_knock/by_room_id/by_event_id.rs`
   - Handler: `pub async fn put(...)`

2. **Parse and validate knock event**
   - Validate event structure per room version
   - Check membership = "knock"
   - Verify signatures

3. **Authorization checks**
   - Room must have join_rule = "knock"
   - User not already in room
   - User not banned
   - Server not denied by ACLs

4. **Add server signature**
   - Sign event with resident server key
   - Add to event.signatures

5. **Persist event**
   - Store in database
   - Update membership to "knock"

6. **Build stripped state response**
   - m.room.name
   - m.room.join_rules
   - m.room.avatar (if present)
   - m.room.topic (if present)

7. **Register route**
   - Add to mod.rs router
   - Add to federation API routes

## Files to Create
- `/packages/server/src/_matrix/federation/v1/send_knock/by_room_id/by_event_id.rs`
- Update `/packages/server/src/_matrix/federation/v1/send_knock/mod.rs`

## Files to Reference
- make_knock: `/packages/server/src/_matrix/federation/v1/make_knock/by_room_id/by_user_id.rs`
- send_join: `/packages/server/src/_matrix/federation/v1/send_join/by_room_id/by_event_id.rs`

## Error Responses

### 403 - Forbidden
```json
{
  "errcode": "M_FORBIDDEN",
  "error": "You are not permitted to knock on this room"
}
```

### 404 - Not Found
```json
{
  "errcode": "M_NOT_FOUND",
  "error": "Unknown room"
}
```

## Verification
- [ ] Endpoint exists and responds
- [ ] Knock event validated
- [ ] Join rules checked (must be "knock")
- [ ] Signature added
- [ ] Event persisted
- [ ] Stripped state returned
- [ ] Error cases handled
- [ ] Works with room versions 7+

## Priority
HIGH - Required for knocking functionality (Matrix v1.1+)
