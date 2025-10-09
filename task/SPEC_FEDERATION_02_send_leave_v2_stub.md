# SPEC_FEDERATION_02: send_leave v2 - Critical Issues

## QA Rating: 6/10

## Status
**FUNCTIONAL BUT NOT PRODUCTION-READY** - Implementation is feature-complete but has critical security and quality issues.

## Implementation Location
`/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v2/send_leave/by_room_id/by_event_id.rs`

---

## CRITICAL ISSUES (Must Fix)

### 1. Security Vulnerability: Signature Validation Bypass Risk
**Location:** Line 103  
**Severity:** CRITICAL

```rust
// CURRENT (INSECURE):
let request_body = serde_json::to_string(&payload).unwrap_or_default();
```

**Problem:** If JSON serialization fails, `unwrap_or_default()` returns an empty string `""`, which is then passed to `validate_server_signature()`. This could allow malicious requests with malformed JSON to bypass signature validation.

**Required Fix:**
```rust
// SECURE VERSION:
let request_body = serde_json::to_string(&payload).map_err(|e| {
    error!("Failed to serialize payload for signature validation: {}", e);
    StatusCode::BAD_REQUEST
})?;
```

**Impact:** This is a potential security vulnerability in federation authentication.

---

### 2. No Test Coverage
**Severity:** CRITICAL

The endpoint has **zero test coverage** despite being a production-critical federation API.

**Required Tests:**
1. **Authentication Tests:**
   - Valid X-Matrix authentication
   - Invalid/missing X-Matrix header
   - Invalid signature
   - Mismatched origin server

2. **Event Validation Tests:**
   - Valid leave event
   - Invalid event type (not m.room.member)
   - Sender != state_key
   - Membership != "leave"
   - Event ID mismatch between path and payload
   - User domain doesn't match origin server

3. **Room and Membership Tests:**
   - Room doesn't exist (404)
   - Federation disabled for room (403)
   - User not in room (403)
   - User already left (400)
   - User is banned (403)
   - Valid leave from join state
   - Valid leave from invite state
   - Valid leave from knock state

4. **PDU Validation Tests:**
   - Valid PDU passes 6-step validation
   - Rejected PDU returns 403
   - Soft-failed PDU is accepted with warning

5. **Signature and Persistence Tests:**
   - Server signature added correctly
   - Event stored in database
   - Membership state updated to Leave
   - Response format is v2 (direct object, not array)

6. **Integration Tests:**
   - End-to-end leave flow
   - Database consistency after leave
   - Error rollback on failure

**Test File Location:** Create `packages/server/tests/federation/v2/send_leave_test.rs`

---

## CODE QUALITY ISSUES (Should Fix)

### 3. Unsafe Signature Handling Patterns
**Location:** Lines 360-373 in `sign_leave_event()`  
**Severity:** MEDIUM

Multiple `unwrap_or_default()` calls could silently mask serialization errors:

```rust
// CURRENT (FRAGILE):
let signatures_value = event
    .signatures
    .as_ref()
    .map(|s| serde_json::to_value(s).unwrap_or_default())  // ⚠️ Silent failure
    .unwrap_or_default();
let mut signatures_map: HashMap<String, HashMap<String, String>> = 
    serde_json::from_value(signatures_value).unwrap_or_default();  // ⚠️ Silent failure
```

**Recommended Fix:**
```rust
// ROBUST VERSION:
let signatures_value = match event.signatures.as_ref() {
    Some(sigs) => serde_json::to_value(sigs)?,
    None => json!({}),
};
let mut signatures_map: HashMap<String, HashMap<String, String>> = 
    serde_json::from_value(signatures_value)?;
```

This ensures serialization errors are properly propagated rather than silently ignored.

---

### 4. Awkward Signature Clearing Pattern
**Location:** Line 324 in `sign_leave_event()`  
**Severity:** LOW

```rust
// CURRENT (UNCLEAR):
event_for_signing.signatures = serde_json::from_value(serde_json::Value::Null).ok();
```

**Recommended Fix:**
```rust
// CLEAR VERSION:
event_for_signing.signatures = None;
```

This is more idiomatic and doesn't rely on JSON serialization round-trip.

---

## Definition of Done

To achieve a 10/10 production-ready rating:

- [ ] **Fix security vulnerability** - Properly handle JSON serialization errors in signature validation
- [ ] **Add comprehensive test suite** - Minimum 15-20 tests covering all scenarios
- [ ] **Fix signature handling** - Use proper error propagation instead of `unwrap_or_default()`
- [ ] **Clean up signature clearing** - Use direct `None` assignment
- [ ] **All tests passing** - `cargo test` completes successfully
- [ ] **No compilation warnings** - Code compiles cleanly

---

## Notes

### What's Complete ✓
- All Matrix Federation API v2 spec requirements implemented
- X-Matrix authentication parsing
- 6-step PDU validation integration
- Event signing with server key
- Database persistence (event + membership)
- Proper v2 response format (`{}` not `[200, {}]`)
- Comprehensive error handling with appropriate HTTP status codes
- Production-quality logging

### What Remains ✗
- Fix critical security vulnerability
- Add comprehensive test coverage
- Improve error handling in signature operations
- Code quality cleanup

---

## Priority
**HIGH** - Security vulnerability must be fixed before production deployment.
