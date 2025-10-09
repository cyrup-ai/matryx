# CLEANUP_02: Remove Fake "Backward Compatibility" from X-Matrix Auth

## STATUS: 8/10 - ONE SEMANTIC ISSUE REMAINS

**Progress**: 4 of 5 changes semantically correct. One critical semantic flaw prevents completion.

---

## OUTSTANDING ISSUE: Change #1 Still Implies Backward Compatibility

### Location
`packages/server/src/auth/x_matrix_parser.rs:118`

### Current (INCORRECT)
```rust
// Accept both "signature" (formal parameter name) and "sig" (shorthand used by older servers) per Matrix spec
```

### Problem
The phrase **"shorthand used by older servers"** STILL IMPLIES BACKWARD COMPATIBILITY. This directly contradicts the entire purpose of this cleanup task.

The task's own research states:
> "Both `sig` (shorthand used in examples) and `signature` (formal parameter name) are legitimate forms."

The Matrix specification's own examples use `sig` as shown in `tmp/matrix-spec/content/server-server-api.md:316`:
```http
Authorization: X-Matrix origin="origin.hs.example.com",destination="destination.hs.example.com",key="ed25519:key1",sig="ABCDEF..."
```

Both forms are **currently valid** per the spec. Neither is deprecated. The word "older" implies legacy/deprecated status.

### Required Fix
Replace with one of these semantically accurate alternatives:

**Option A (Recommended):**
```rust
// Accept both "signature" (formal parameter name) and "sig" (shorthand form per Matrix spec)
```

**Option B:**
```rust
// Accept both "signature" (formal parameter name) and "sig" (shorthand used in Matrix spec examples)
```

**Option C:**
```rust
// Accept both "signature" (formal parameter name) and "sig" (alternative form per Matrix spec)
```

The key requirement: **Remove "older servers"** - it must not imply temporal/version-based support.

---

## COMPLETED ITEMS (DO NOT MODIFY)

✅ Change #2 (x_matrix_parser.rs:126): "Destination parameter is optional per Matrix spec"  
✅ Change #3 (x_matrix_parser.rs:288): "Test compatibility with "sig" parameter (shorthand accepted by Matrix spec)"  
✅ Change #4 (middleware.rs:169): "X-Matrix request without destination parameter (optional per Matrix spec)"  
✅ Change #5 (middleware.rs:265): "No X-Matrix-Token header present (optional per Matrix spec)"

---

## DEFINITION OF DONE

1. ✅ All 5 changes implemented (Change #1 needs semantic refinement)
2. ⏳ Change #1 must not imply backward compatibility or temporal version support
3. ⏳ Code compiles successfully (currently blocked by unrelated PresenceRepository error)
4. ⏳ No new warnings introduced

---

## QA RATING BREAKDOWN

**Technical Execution**: 10/10 - All locations updated, clean implementation  
**Semantic Accuracy**: 6/10 - Change #1 still implies backward compatibility  
**Code Quality**: 10/10 - Professional, well-structured  
**Completeness**: 8/10 - 4 of 5 changes semantically correct

**OVERALL**: 8/10

**Blockers**:
- Semantic: Change #1 phrase "used by older servers" contradicts task objective
- Compilation: Unrelated PresenceRepository::create_presence_live_query error (not this task's responsibility)

---

## RESOLUTION STEPS

1. Edit line 118 in `packages/server/src/auth/x_matrix_parser.rs`
2. Replace "shorthand used by older servers" with "shorthand form per Matrix spec" (or similar neutral language)
3. Verify: `grep -n "older servers" packages/server/src/auth/x_matrix_parser.rs` returns nothing
4. Delete this task file once semantic issue resolved
