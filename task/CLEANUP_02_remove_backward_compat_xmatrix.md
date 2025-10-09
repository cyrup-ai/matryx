# CLEANUP_02: Remove Fake "Backward Compatibility" from X-Matrix Auth

## STATUS: BLOCKED - Unrelated Compilation Error

### ✅ COMPLETED CHANGES

All 5 required X-Matrix comment/log message updates have been successfully implemented:

1. ✅ `x_matrix_parser.rs:119` - Changed to "Accept both "signature" (formal parameter name) and "sig" (shorthand used by older servers) per Matrix spec"
2. ✅ `x_matrix_parser.rs:126` - Changed to "Destination parameter is optional per Matrix spec"  
3. ✅ `x_matrix_parser.rs:288` - Changed to "Test compatibility with "sig" parameter (shorthand accepted by Matrix spec)"
4. ✅ `middleware.rs:170` - Changed to "X-Matrix request without destination parameter (optional per Matrix spec)"
5. ✅ `middleware.rs:266` - Changed to "No X-Matrix-Token header present (optional per Matrix spec)"

**Quality Assessment:** All changes are semantically correct and accurately reflect Matrix protocol compliance instead of misleading "backward compatibility" terminology.

### ❌ BLOCKING ISSUE

**Definition of Done Requirements:**
- [ ] Code compiles without errors: `cargo check -p matryx_server` - **BLOCKED**
- [ ] No new warnings introduced: `cargo clippy -p matryx_server` - Cannot verify due to compilation failure

**Compilation Error:**
```
error[E0599]: no method named `create_presence_live_query` found for struct `PresenceRepository`
  --> packages/server/src/_matrix/client/v3/sync/streaming/presence_streams.rs:20:36
   |
20 |     let mut stream = presence_repo.create_presence_live_query(&user_id).await?;
   |                                    ^^^^^^^^^^^^^^^^^^^^^^^^^^ method not found in `PresenceRepository`
```

**Root Cause:** The `PresenceRepository` is missing the `create_presence_live_query` method that is being called in the streaming presence implementation. This is completely unrelated to the X-Matrix authentication cleanup.

**Action Required:** Fix the presence repository implementation to add the missing method before the X-Matrix task can be marked as complete.

## NEXT STEPS

1. Implement `create_presence_live_query` method in `PresenceRepository` 
2. Verify `cargo check -p matryx_server` passes
3. Verify `cargo clippy -p matryx_server` shows no new warnings
4. Delete this task file once Definition of Done is met

## TECHNICAL NOTES

The X-Matrix cleanup is 100% complete and correct. The blocking issue is a missing repository method in an unrelated subsystem (presence streaming). Once the presence repository is fixed, this task will be complete.
