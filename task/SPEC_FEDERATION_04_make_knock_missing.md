# SPEC_FEDERATION_04: Implement make_knock Endpoint

## Status  
MISSING - Endpoint does not exist

## Description
The `/make_knock` endpoint is missing. This is the first step in the knock handshake (added in Matrix v1.1).

## Spec Requirements (spec/server/12-room-knocking.md)

### Endpoint
`GET /_matrix/federation/v1/make_knock/{roomId}/{userId}`

### What's Required
1. Validate room allows knocking
2. Check user not already in room
3. Check user not banned
4. Generate unsigned knock event template
5. Return template with room version

### Query Parameters
- `ver`: Array of supported room versions (required)

### Response Format (200)
```json
{
  "event": {
    "content": {"membership": "knock"},
    "origin": "example.org",
    "origin_server_ts": 1549041175876,
    "room_id": "!somewhere:example.org",
    "sender": "@someone:example.org",
    "state_key": "@someone:example.org",
    "type": "m.room.member"
  },
  "room_version": "7"
}
```

## What Needs to be Done

1. **Create endpoint file**
   - Path: `/packages/server/src/_matrix/federation/v1/make_knock/by_room_id/by_user_id.rs`
   - Handler: `pub async fn get(...)`

2. **Parse query parameters**
   - Extract `ver` parameter (required)
   - Validate room version compatibility

3. **Validate request**
   - Parse X-Matrix auth
   - Verify user belongs to origin server
   - Room must exist
   - Server not denied by ACLs

4. **Authorization checks**
   - Room must have join_rule = "knock"  
   - User not already member
   - User not banned
   - Room version must be 7+

5. **Build event template**
   - Create unsigned knock event
   - Set membership to "knock"
   - Include auth_events
   - Include prev_events

6. **Return response**
   - Include event template
   - Include room_version

7. **Register route**
   - Add to mod.rs
   - Wire into router

## Files to Create
- `/packages/server/src/_matrix/federation/v1/make_knock/by_room_id/by_user_id.rs`
- Update `/packages/server/src/_matrix/federation/v1/make_knock/mod.rs`

## Files to Reference
- make_join: `/packages/server/src/_matrix/federation/v1/make_join/by_room_id/by_user_id.rs`

## Error Responses

### 400 - Bad Request (incompatible version)
```json
{
  "errcode": "M_INCOMPATIBLE_ROOM_VERSION",
  "error": "Your homeserver does not support the features required to knock on this room",
  "room_version": "7"
}
```

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
- [ ] Query parameter `ver` parsed
- [ ] Room version compatibility checked
- [ ] Join rules validated (must be "knock")
- [ ] User authorization checked
- [ ] Event template generated
- [ ] auth_events included
- [ ] prev_events included
- [ ] Room version returned
- [ ] Error cases handled
- [ ] Only works with room version 7+

## Priority
HIGH - Required for knocking functionality (Matrix v1.1+)
