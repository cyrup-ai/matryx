# SPEC_FEDERATION_02: send_leave v2 - CRITICAL BUG FOUND

## QA Rating: 3/10

## Status
**PRODUCTION-BLOCKING BUG** - Critical defect at final step causes ALL valid leave requests to fail with 500 error.

## Implementation Location
`/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v2/send_leave/by_room_id/by_event_id.rs`

---

## CRITICAL BUG 🚨

### Bug Location: Line 308

**Current (WRONG):**
```rust
membership_repo.create(&updated_membership).await.map_err(|e| {
    error!("Failed to update membership record: {}", e);
    StatusCode::INTERNAL_SERVER_ERROR
})?;
```

**Should be:**
```rust
membership_repo.update(&updated_membership).await.map_err(|e| {
    error!("Failed to update membership record: {}", e);
    StatusCode::INTERNAL_SERVER_ERROR
})?;
```

### Why This is Critical

**Root Cause:**
- Code already verified membership exists (lines 209-238)
- User can only leave from Join/Invite/Knock states (existing membership required)
- SurrealDB's `.create()` with existing ID `room_id:user_id` **FAILS** if record exists
- Must use `.update()` for existing records

**Impact:**
- **100% failure rate** for ALL valid leave requests
- Failure occurs AFTER successful PDU validation, event signing, and event storage
- Returns HTTP 500 instead of successful leave
- Event is stored but membership state never updates to Leave
- Database inconsistency: event stored but membership unchanged

**Evidence:**
- Reference pattern in `packages/surrealdb/src/repository/membership.rs:1149`:
  ```rust
  pub async fn leave_room(...) -> Result<(), RepositoryError> {
      // ... validation ...
      self.update_membership(&membership).await?;  // ✅ CORRECT
      Ok(())
  }
  ```

### Required Fix

**Single-line change at line 308:**
```diff
- membership_repo.create(&updated_membership).await.map_err(|e| {
+ membership_repo.update(&updated_membership).await.map_err(|e| {
```

**Verification:**
1. Change `.create()` to `.update()` at line 308
2. Recompile: `cargo build -p matryx_server`
3. Run tests: `cargo test -p matryx_server --test federation`
4. Verify no compilation errors or test failures

---

## SECONDARY ISSUE: Test Coverage Gap

### Issue
Tests have comprehensive negative path coverage (17 tests) but ZERO positive path coverage:
- All tests use invalid signatures and expect failures
- No test reaches line 308 (membership update)
- Bug went undetected because no test verifies successful leave completion

### Recommendation
**OPTIONAL (not blocking 10/10 rating):**
Add at least one integration test that:
1. Sets up valid cryptographic signatures
2. Completes full leave flow including PDU validation
3. Verifies membership state changes to Leave
4. Confirms event is stored correctly

This would catch similar bugs in the future but is not required for production deployment once the critical bug is fixed.

---

## Definition of Done for 10/10

- [X] Security vulnerability fixed ✅ COMPLETE (line 103-106)
- [X] Signature handling with proper error propagation ✅ COMPLETE (lines 368-375)
- [X] Signature clearing uses None ✅ COMPLETE (line 351)
- [X] Comprehensive negative path test coverage ✅ COMPLETE (17 tests)
- [ ] **Fix line 308: Change .create() to .update()** ← BLOCKING ISSUE
- [ ] Verify fix compiles and tests pass

---

## What Was Verified Complete ✅

The following items from previous review are CONFIRMED complete and production-ready:

1. **Security Fix**: Serialization error handling (lines 103-106) properly uses `.map_err()`
2. **Signature Handling**: Robust error propagation (lines 368-375) uses `?` operator  
3. **Signature Clearing**: Clean implementation (line 351) uses `None` assignment
4. **Authentication Tests**: 5 comprehensive tests for X-Matrix validation
5. **Event Validation Tests**: 6 comprehensive tests for event structure  
6. **State Validation Tests**: 6 comprehensive tests for membership states
7. **Code Quality**: Production-quality logging, error handling, HTTP status codes
8. **v2 Response Format**: Correct (line 310) returns `json!({})` not `[200, {}]`
9. **PDU Validation**: Properly integrated (lines 240-264) with 6-step pipeline
10. **Event Signing**: Correct implementation (lines 320-388) with proper key handling

---

## Priority
**CRITICAL** - Single-line fix required for production deployment.

## Recommendation
Fix the one-line bug at line 308, verify compilation and tests pass, then deploy. All other implementation is production-ready.
