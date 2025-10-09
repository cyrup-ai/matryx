# SPEC_FEDERATION_08: Implement 3PID onbind Endpoint

## Status
MISSING - Endpoint does not exist

## Description
The 3PID onbind endpoint is missing. Identity servers use this to notify homeservers when a third-party identifier gets bound to a Matrix ID.

## Spec Requirements (spec/server/11-room-invites.md)

### Endpoint
`PUT /_matrix/federation/v1/3pid/onbind`

### What's Required
1. Receive notification from identity server
2. Process pending room invites for the bound 3PID
3. Create proper invite events
4. No authentication required (identity server callback)

### Request Body
```json
{
  "address": "alice@example.com",
  "invites": [
    {
      "address": "alice@example.com",
      "medium": "email",
      "mxid": "@alice:matrix.org",
      "room_id": "!somewhere:example.org",
      "sender": "@bob:matrix.org",
      "signed": {
        "mxid": "@alice:example.org",
        "signatures": {
          "magic.forest": {
            "ed25519:3": "base64_signature"
          }
        },
        "token": "abc123"
      }
    }
  ],
  "medium": "email",
  "mxid": "@alice:matrix.org"
}
```

### Response Format (200)
```json
{}
```

## What Needs to be Done

1. **Create endpoint file**
   - Path: `/packages/server/src/_matrix/federation/v1/3pid/onbind.rs`
   - Handler: `pub async fn put(...)`
   - No authentication required!

2. **Parse request body**
   - Extract address
   - Extract medium (email/msisdn)
   - Extract mxid
   - Extract invites array

3. **Validate mxid**
   - Verify mxid belongs to our server
   - Check user exists

4. **Process each invite**
   - Verify signed object
   - Check token matches m.room.third_party_invite
   - Verify identity server signature
   - Check room exists

5. **Create invite events**
   - Build m.room.member invite event
   - Include third_party_invite object
   - Set membership to "invite"
   - Add content.third_party_invite.signed

6. **Exchange invites**
   - If we're in the room, process directly
   - If not, use /exchange_third_party_invite
   - Send to inviting server

7. **Return success**
   - Return empty object {}

8. **Register route**
   - Add to 3pid/mod.rs
   - Wire into router
   - NO AUTH MIDDLEWARE!

## Files to Create
- `/packages/server/src/_matrix/federation/v1/3pid/onbind.rs`
- Update `/packages/server/src/_matrix/federation/v1/3pid/mod.rs`

## Files to Reference
- exchange_third_party_invite: `/packages/server/src/_matrix/federation/v1/exchange_third_party_invite/by_room_id.rs`
- Identity server integration

## Security Considerations
- NO authentication (identity server callback)
- MUST verify identity server signatures
- MUST validate token against stored third_party_invite
- Rate limit to prevent abuse
- Validate mxid belongs to our server

## Verification
- [ ] Endpoint exists (no auth!)
- [ ] Request body parsed
- [ ] mxid validated (our server)
- [ ] Signature verified
- [ ] Token validated
- [ ] Invite event created
- [ ] third_party_invite included
- [ ] Exchange or direct process
- [ ] Returns {}

## Priority
MEDIUM - Required for email/phone invites
