# SPEC_FEDERATION_11: Verify Server Key Query Implementation

## Status
EXISTS - Needs verification against spec

## Description
Server key query endpoints exist but need verification for proper key retrieval and caching.

## Spec Requirements (spec/server/03-server-keys.md)

### Endpoints
1. `GET /_matrix/key/v2/server` - Get this server's keys
2. `GET /_matrix/key/v2/server/{keyId}` - Get specific key
3. `POST /_matrix/key/v2/query` - Query keys from multiple servers
4. `POST /_matrix/key/v2/query/{serverName}` - Query specific server keys

### What's Required
- Return server signing keys
- Include validity periods
- Support key rotation
- Cache remote keys
- Verify key signatures
- Handle expired keys

### Response Format
```json
{
  "server_name": "example.org",
  "verify_keys": {
    "ed25519:auto": {
      "key": "Base64EncodedKey"
    }
  },
  "old_verify_keys": {
    "ed25519:old": {
      "key": "Base64EncodedOldKey",
      "expired_ts": 1234567890
    }
  },
  "valid_until_ts": 1234567890,
  "signatures": {
    "example.org": {
      "ed25519:auto": "Base64Signature"
    }
  }
}
```

## Current Implementation
**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/key/v2/`

Files to verify:
- server.rs
- query/by_server_name.rs
- query/mod.rs

## What Needs Verification

1. **Key Generation**
   - [ ] Ed25519 keys generated
   - [ ] Keys properly signed
   - [ ] Validity periods set

2. **Key Storage**
   - [ ] Current keys stored
   - [ ] Old keys preserved
   - [ ] Expiry tracked

3. **Key Retrieval**
   - [ ] GET /server works
   - [ ] Specific key lookup
   - [ ] Batch query supported

4. **Key Caching**
   - [ ] Remote keys cached
   - [ ] Cache expiry respected
   - [ ] Re-fetch on expiry

5. **Key Verification**
   - [ ] Self-signatures valid
   - [ ] Expiry checked
   - [ ] Rotation handled

## Verification Checklist
- [ ] GET /server returns valid keys
- [ ] verify_keys populated
- [ ] old_verify_keys handled
- [ ] valid_until_ts set correctly
- [ ] Self-signed properly
- [ ] Query endpoint works
- [ ] Batch queries supported
- [ ] Remote key caching works
- [ ] Expired keys handled

## Priority
HIGH - Critical for signature verification
