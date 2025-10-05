# STUB_5: Old Verify Keys Implementation

## OBJECTIVE

Implement historical server signing key tracking (old_verify_keys) to support proper cryptographic key rotation in the Matrix federation protocol. Currently, old verification keys are not tracked, breaking the federation trust model when keys are rotated.

## SEVERITY

**CRITICAL SECURITY ISSUE**

## LOCATION

- **Primary File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/key/v2/server.rs:133`

## CURRENT STUB CODE

```rust
// Build old_verify_keys JSON (empty for now)
let old_verify_keys = json!({});
```

## SUBTASKS

### SUBTASK1: Understand Matrix Key Rotation

**What:** Research Matrix server key rotation requirements  
**Where:** Matrix Server-Server specification  
**Why:** Need to understand what old_verify_keys are used for  

**Requirements:**
- Download Matrix spec on server key management
- Save to `/Volumes/samsung_t9/maxtryx/docs/matrix-key-rotation.md`
- Understand when keys are rotated
- Understand how old keys are used (verifying old signatures)
- Document the old_verify_keys JSON format
- Understand validity periods and expiration

### SUBTASK2: Design Key Storage Schema

**What:** Add database schema for storing historical server keys  
**Where:** `/Volumes/samsung_t9/maxtryx/packages/surrealdb/migrations/matryx.surql`  
**Why:** Need persistent storage for key history  

**Requirements:**
- Add server_key_history table or extend existing server_key table
- Fields needed:
  - `key_id` (e.g., "ed25519:1")
  - `public_key` (base64 encoded)
  - `valid_from` (timestamp when key became active)
  - `valid_until` (timestamp when key was rotated out)
  - `server_name` (for the local server)
- Consider separate current_key vs old_keys tables
- Add indexes for efficient querying

### SUBTASK3: Create Key History Entity

**What:** Add entity type for key history  
**Where:** `/Volumes/samsung_t9/maxtryx/packages/entity/src/`  
**Why:** Need type-safe representation  

**Requirements:**
- Create ServerKeyHistory or OldVerifyKey struct
- Match schema from SUBTASK2
- Implement serialization for Matrix JSON format
- Follow existing entity patterns

### SUBTASK4: Implement Key History Repository

**What:** Add repository methods for key management  
**Where:** `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/`  
**Why:** Need data access layer  

**Requirements:**
- Create or extend existing key repository
- Methods needed:
  - `store_current_key(key_id, public_key, valid_from) -> Result<()>`
  - `rotate_key(old_key_id, new_key_id, new_public_key) -> Result<()>`
  - `get_old_verify_keys() -> Result<Vec<OldVerifyKey>>`
  - `get_key_by_id(key_id) -> Result<ServerKey>` (for verifying old signatures)
- Handle key rotation atomically
- Ensure valid_until is set when rotating

### SUBTASK5: Update Server Key Endpoint

**What:** Return old_verify_keys in key server response  
**Where:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/key/v2/server.rs:133`  
**Why:** Federation servers need historical keys  

**Requirements:**
- Remove stub code
- Call repository to get old verify keys
- Build JSON map of old keys with proper format
- Include validity timestamps per Matrix spec
- Ensure response follows Matrix key server format

### SUBTASK6: Implement Key Rotation Command

**What:** Create mechanism to rotate server signing keys  
**Where:** New admin endpoint or CLI command  
**Why:** Need operational capability to rotate keys  

**Requirements:**
- Create admin endpoint or CLI command for key rotation
- Generate new ed25519 key pair
- Store new key as current
- Move old current key to old_verify_keys
- Update server configuration
- Ensure atomic operation (no gap in key coverage)

## DEFINITION OF DONE

- [ ] Database schema for key history added
- [ ] Entity types created
- [ ] Repository methods implemented
- [ ] Server key endpoint returns old_verify_keys
- [ ] Key rotation mechanism implemented
- [ ] Stub comment removed
- [ ] Code compiles without errors
- [ ] Current key and old keys properly separated

## RESEARCH NOTES

### Matrix Key Server Format

**Expected JSON structure:**
```json
{
  "server_name": "example.com",
  "valid_until_ts": 1234567890,
  "verify_keys": {
    "ed25519:current": {
      "key": "base64..."
    }
  },
  "old_verify_keys": {
    "ed25519:old1": {
      "key": "base64...",
      "expired_ts": 1234567890
    }
  }
}
```

### Ed25519 Keys

**Already in dependencies:**
- `ed25519-dalek` crate
- Review key generation and serialization
- Understand base64 encoding requirements

### Why This Matters

**Security implications:**
- Other servers verify event signatures using keys
- When key is rotated, old events still have old signatures
- Old keys must be published so old events can still be verified
- Without old_verify_keys, key rotation breaks federation

## RELATIONSHIP TO OTHER STUBS

- **STUB_2** (Federation Invite Verification) - Needs key fetching infrastructure
- This task provides the server's own key history
- STUB_2 consumes other servers' key history

## NO TESTS OR BENCHMARKS

Do NOT write unit tests, integration tests, or benchmarks as part of this task. The testing team will handle test coverage separately.

---

## MATRIX SPECIFICATION REQUIREMENTS

### Server Key Management - Old Verify Keys

From `/spec/server/03-server-keys.md`:

**Publishing Keys:**

> Homeservers publish their signing keys in a JSON object at `/_matrix/key/v2/server`. The response contains a list of `verify_keys` that are valid for signing federation requests made by the homeserver and for signing events. It contains a list of `old_verify_keys` which are only valid for signing events.

**Response Structure:**

```json
{
  "old_verify_keys": {
    "ed25519:0ldk3y": {
      "expired_ts": 1532645052628,
      "key": "VGhpcyBzaG91bGQgYmUgeW91ciBvbGQga2V5J3MgZWQyNTUxOSBwYXlsb2FkLg"
    }
  },
  "server_name": "example.org",
  "signatures": { /* ... */ },
  "valid_until_ts": 1652262000000,
  "verify_keys": {
    "ed25519:abc123": {
      "key": "VGhpcyBzaG91bGQgYmUgYSByZWFsIGVkMjU1MTkgcGF5bG9hZA"
    }
  }
}
```

**Field Requirements:**

- `verify_keys` (object, required): Current public keys for verifying digital signatures
  - Key format: algorithm:version (e.g., `ed25519:abc123`)
  - Version must match: `[a-zA-Z0-9_]`
  - Contains `key` field with Unpadded base64 encoded key

- `old_verify_keys` (object): Historical public keys no longer used for new signatures
  - Same key format as verify_keys
  - **MUST include `expired_ts`**: POSIX timestamp in milliseconds when key stopped being used
  - Still valid for verifying old event signatures

- `valid_until_ts` (integer, required): POSIX timestamp when keys should be refreshed
  - Servers MUST use lesser of this field and 7 days into future
  - Keys used beyond this timestamp MUST be considered invalid (room version dependent)

**Key Rotation Behavior:**

1. Server generates new signing key
2. New key added to `verify_keys`
3. Old current key moved to `old_verify_keys` with `expired_ts` set
4. Old events signed with old key remain verifiable
5. New events MUST be signed with current key only

**Why Old Keys Matter:**

- Federation events have long lifetimes
- Old events still need signature verification
- Room history must remain verifiable after key rotation
- Without old_verify_keys, key rotation breaks federation trust

**Security Requirements:**

1. **MUST publish** old keys for event verification
2. **MUST set** accurate expired_ts timestamps
3. **MUST NOT use** old keys for new signatures
4. **MUST keep** old keys available for reasonable retention period
5. **MUST sign** the entire key response including old_verify_keys
