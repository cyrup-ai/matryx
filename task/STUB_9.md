# STUB_9: Third-Party Invite Signature Validation

## OBJECTIVE

Implement cryptographic validation of third-party signed invitations from identity servers. Currently, invitations signed by identity servers are logged but not validated, creating an authentication bypass vulnerability where unsigned or forged invitations could be accepted.

## SEVERITY

**CRITICAL SECURITY ISSUE**

## LOCATION

- **Primary File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/join/by_room_id_or_alias.rs:95`

## CURRENT STUB CODE

```rust
// TODO: Implement proper third-party signed invitation validation
// This involves verifying cryptographic signatures from identity servers
// For now, we log the presence but proceed with standard join
```

## SUBTASKS

### SUBTASK1: Understand Third-Party Invitations

**What:** Research Matrix third-party invitation mechanism  
**Where:** Matrix Client-Server specification  
**Why:** Need to understand the complete flow and data structures  

**Requirements:**
- Download Matrix spec on third-party invitations
- Save to `/Volumes/samsung_t9/maxtryx/docs/matrix-third-party-invites.md`
- Document the invitation flow:
  - Identity server role
  - Signature format
  - Validation requirements
- Understand the `third_party_signed` parameter structure
- Document public key discovery for identity servers

### SUBTASK2: Review Current Join Implementation

**What:** Understand the existing join endpoint code  
**Where:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/join/by_room_id_or_alias.rs`  
**Why:** Need context for where validation fits  

**Requirements:**
- Read the complete file
- Understand the join flow
- Locate where third_party_signed is currently detected
- Understand current error handling
- Document the request/response structures

### SUBTASK3: Define Third-Party Signed Data Structure

**What:** Create or locate entity for third_party_signed data  
**Where:** `/Volumes/samsung_t9/maxtryx/packages/entity/src/`  
**Why:** Need type-safe representation  

**Requirements:**
- Define or find ThirdPartySigned struct
- Fields typically include:
  - `sender` (the inviter's user ID)
  - `mxid` (the invitee's Matrix ID)
  - `token` (invitation token)
  - `signatures` (identity server signatures)
- Follow Matrix spec format exactly
- Implement deserialization

### SUBTASK4: Implement Identity Server Key Fetching

**What:** Add functionality to fetch identity server public keys  
**Where:** New module or extend existing federation key fetching  
**Why:** Need public keys to verify signatures  

**Requirements:**
- Implement identity server key discovery
- Fetch public keys from identity server
- Cache keys appropriately
- Handle key fetch failures
- Reuse patterns from federation key fetching if possible (see STUB_2)

### SUBTASK5: Implement Signature Verification

**What:** Verify signatures on third-party invitations  
**Where:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/join/by_room_id_or_alias.rs:95`  
**Why:** Validate invitation authenticity  

**Requirements:**
- Remove TODO comment
- Extract third_party_signed from request
- Fetch identity server public key
- Verify signature on invitation data
- Use ed25519-dalek crate (already in dependencies)
- Handle verification failures with appropriate Matrix errors

### SUBTASK6: Enforce Validation Policy

**What:** Define and implement validation requirements  
**Where:** Same file as SUBTASK5  
**Why:** Determine what happens on validation failure  

**Requirements:**
- If third_party_signed is present, signature MUST be valid
- Reject join if signature validation fails
- Return M_FORBIDDEN with descriptive error
- Log validation failures for security monitoring
- Allow join to proceed only if:
  - No third_party_signed (normal join), OR
  - third_party_signed present AND valid

## DEFINITION OF DONE

- [ ] Third-party invitation spec requirements documented
- [ ] ThirdPartySigned entity defined
- [ ] Identity server key fetching implemented
- [ ] Signature verification implemented
- [ ] Validation enforced in join flow
- [ ] TODO comment removed
- [ ] Appropriate errors returned on validation failure
- [ ] Code compiles without errors

## RESEARCH NOTES

### Third-Party Invitation Flow

**Typical sequence:**
1. User is invited via email/phone through identity server
2. Identity server creates signed invite token
3. User joins room using token
4. Homeserver validates identity server signature
5. Join proceeds if signature valid

### Signature Format

**Expected structure:**
```json
{
  "third_party_signed": {
    "sender": "@inviter:example.com",
    "mxid": "@invitee:example.com",
    "token": "random_token",
    "signatures": {
      "identity.server.com": {
        "ed25519:0": "signature_here"
      }
    }
  }
}
```

### Identity Server Trust

**Security considerations:**
- Which identity servers are trusted?
- How to prevent malicious identity servers?
- Should there be a whitelist?
- Consider server configuration for trusted identity servers

### Related Code

**Look for:**
- Existing identity server integration
- Similar signature verification (federation, STUB_2)
- Key caching infrastructure

## RELATIONSHIP TO OTHER STUBS

- **STUB_2** (Federation Invite Verification) - Similar signature verification patterns
- May share key fetching and verification code

## NO TESTS OR BENCHMARKS

Do NOT write unit tests, integration tests, or benchmarks as part of this task. The testing team will handle test coverage separately.

---

## MATRIX SPECIFICATION REQUIREMENTS

### Third-Party Invitations and Identity Server Signatures

From `/spec/client/01_foundation_api.md`:

**m.room.member Event with Third-Party Invite:**

```json
{
  "content": {
    "displayname": "Alice Margatroid",
    "membership": "invite",
    "third_party_invite": {
      "display_name": "alice",
      "signed": {
        "mxid": "@alice:example.org",
        "signatures": {
          "magic.forest": {
            "ed25519:3": "fQpGIW1Snz+pwLZu6sTy2aHy/DYWWTspTJRPyNp0PKkymfIsNffysMl6ObMMFdIJhk6g6pwlIqZ54rxo8SLmAg"
          }
        },
        "token": "abc123"
      }
    }
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@example:example.org",
  "state_key": "@alice:example.org",
  "type": "m.room.member",
  "unsigned": {
    "age": 1234
  }
}
```

**Third-Party Invite Structure:**

**`third_party_invite` object:**
- `display_name` (string, required): Name to represent the user
- `signed` (SignedThirdPartyInvite, required): Block signed by identity server

**`SignedThirdPartyInvite` object:**
- `mxid` (User ID, required): User ID bound to third-party identifier
- `signatures` (object, required): Identity server signatures
  - Map of identity server name → signing key ID → base64 signature
  - Calculated using Signing JSON process
- `token` (string, required): Token from identity server `/store_invite` endpoint
  - Matches state_key of m.room.third_party_invite event

**Third-Party Invite Property:**

> The `third_party_invite` property will be set if this invite is an `invite` event and is the successor of an `m.room.third_party_invite` event, and absent otherwise.

**Identity Server Trust:**

From error codes:

> `M_SERVER_NOT_TRUSTED` - The client's request used a third-party server, e.g. identity server, that this server does not trust.

**Signature Verification Requirements:**

1. **MUST verify** signatures from identity server
2. **MUST validate** token matches m.room.third_party_invite
3. **MUST check** identity server is trusted
4. **MUST use** ed25519 signature verification
5. **MUST apply** canonical JSON rules

**Authorization Flow:**

From `/spec/server/11-room-invites.md`:

> **Third-party Authorization**
> - Identity server signatures must be validated
> - Token must match stored third-party invite

**Signature Format:**

- Algorithm: ed25519
- Key format: `ed25519:3` (algorithm:version)
- Signature: Base64-encoded
- Signed content: Canonical JSON of invite data

**Error Handling:**

- `M_FORBIDDEN` - Signature verification failed
- `M_SERVER_NOT_TRUSTED` - Identity server not trusted
- `M_UNAUTHORIZED` - Invalid signature

**Security Considerations:**

1. Only trust whitelisted identity servers
2. Validate all cryptographic signatures
3. Ensure token hasn't been reused
4. Check identity server key validity
5. Prevent replay attacks
