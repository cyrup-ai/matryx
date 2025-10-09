# SPEC_FEDERATION_05: Implement invite v2 Endpoint

## Status
MISSING - v2 endpoint does not exist

## Description
The v2 invite endpoint is missing. This is the preferred invite API over v1 (provides room_version explicitly).

## Spec Requirements (spec/server/11-room-invites.md)

### Endpoint
`PUT /_matrix/federation/v2/invite/{roomId}/{eventId}`

### What's Required
1. Accept invite event from inviting server
2. Validate invite event structure
3. Add invited server's signature
4. Return signed event
5. More standardized than v1

### Request Body
```json
{
  "event": {
    "content": {"membership": "invite"},
    "origin": "matrix.org",
    "origin_server_ts": 1234567890,
    "sender": "@someone:example.org",
    "state_key": "@joe:elsewhere.com",
    "type": "m.room.member"
  },
  "invite_room_state": [
    {
      "content": {"name": "Example Room"},
      "sender": "@bob:example.org",
      "state_key": "",
      "type": "m.room.name"
    }
  ],
  "room_version": "2"
}
```

### Response Format (200)
```json
{
  "event": {
    "content": {"membership": "invite"},
    "origin": "example.org",
    "origin_server_ts": 1549041175876,
    "room_id": "!somewhere:example.org",
    "sender": "@someone:example.org",
    "signatures": {
      "elsewhere.com": {
        "ed25519:k3y_versi0n": "SomeOtherSignatureHere"
      },
      "example.com": {
        "ed25519:key_version": "SomeSignatureHere"
      }
    },
    "state_key": "@someone:example.org",
    "type": "m.room.member"
  }
}
```

## What Needs to be Done

1. **Create endpoint file**
   - Path: `/packages/server/src/_matrix/federation/v2/invite/by_room_id/by_event_id.rs`
   - Handler: `pub async fn put(...)`

2. **Parse request body**
   - Extract event object
   - Extract invite_room_state
   - Extract room_version

3. **Validate invite event**
   - Check event structure per room_version
   - Verify membership = "invite"
   - Check target user belongs to our server
   - Validate signatures

4. **Authorization checks**
   - Verify sender has permission to invite
   - Check room allows invites
   - User not already in room
   - User not banned
   - Server accepts invites

5. **Add server signature**
   - Sign event with invited server's key
   - Add to event.signatures
   - Keep all other fields unchanged

6. **Store invite**
   - Persist to database
   - Store invite_room_state for client

7. **Return signed event**
   - Return event with added signature
   - Include unsigned.invite_room_state

8. **Register route**
   - Add to mod.rs
   - Wire into router

## Files to Create
- `/packages/server/src/_matrix/federation/v2/invite/by_room_id/by_event_id.rs`
- Update `/packages/server/src/_matrix/federation/v2/invite/mod.rs`

## Files to Reference
- v1 invite: `/packages/server/src/_matrix/federation/v1/invite/by_room_id/by_event_id.rs`
- Event signing: `/packages/server/src/federation/event_signer.rs`

## Error Responses

### 400 - Bad Request
```json
{
  "errcode": "M_BAD_JSON",
  "error": "Invalid room version or event structure"
}
```

### 403 - Forbidden
```json
{
  "errcode": "M_FORBIDDEN",
  "error": "The invite is not allowed"
}
```

## Verification
- [ ] Endpoint exists and responds
- [ ] Request body parsed correctly
- [ ] room_version used for validation
- [ ] Event validated per room version
- [ ] Signature added correctly
- [ ] Original fields preserved
- [ ] Event persisted
- [ ] invite_room_state stored
- [ ] Error cases handled
- [ ] Fallback to v1 if 400/404

## Priority
HIGH - Preferred over v1, required for modern federation
