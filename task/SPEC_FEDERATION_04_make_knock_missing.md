# SPEC_FEDERATION_04: Fix knock_restricted Join Rule Support

## Status  
🔧 **BUG FIX REQUIRED** - Implementation exists but has a critical logic bug

---

## QA Review Rating: 8/10

### Implementation Status

✅ **COMPLETE**: Both `make_knock` and `send_knock` endpoints are fully implemented  
✅ **COMPLETE**: Router registration and module hierarchy  
✅ **COMPLETE**: All supporting infrastructure (helper functions, database methods)  
✅ **COMPLETE**: Comprehensive validation and error handling  
✅ **COMPLETE**: Matrix specification compliance  

❌ **BUG**: `knock_restricted` join rule not supported in database validation

---

## Critical Bug

**Location**: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/room.rs:2397`

**Method**: `RoomRepository::check_room_allows_knocking()`

**Issue**: The method only checks for `"knock"` join rule but fails to check for `"knock_restricted"` as required by the Matrix specification.

**Current Code** (line 2397):
```rust
match join_rules {
    Some(rules) => {
        let join_rule = rules.join_rule.unwrap_or_else(|| "invite".to_string());
        Ok(join_rule == "knock")  // ❌ ONLY checks "knock"
    },
    None => {
        Ok(false)
    },
}
```

**Required Fix**:
```rust
match join_rules {
    Some(rules) => {
        let join_rule = rules.join_rule.unwrap_or_else(|| "invite".to_string());
        Ok(matches!(join_rule.as_str(), "knock" | "knock_restricted"))  // ✅ Checks both
    },
    None => {
        Ok(false)
    },
}
```

**Why This Matters**: 
- The Matrix specification supports both `"knock"` and `"knock_restricted"` join rules for knocking
- The helper function `room_supports_knocking()` in `membership_federation.rs` correctly checks both values
- This inconsistency causes the endpoint to reject valid knock requests for rooms with `knock_restricted` join rules

---

## Verification Steps

After fixing the bug:

1. **Verify the fix compiles**:
   ```bash
   cargo check -p matryx_surrealdb
   ```

2. **Check consistency** with helper function at `packages/server/src/federation/membership_federation.rs:1011`:
   ```rust
   fn room_supports_knocking(join_rules: &str) -> bool {
       matches!(join_rules, "knock" | "knock_restricted")
   }
   ```

3. **Test both join rule types**:
   - Rooms with `join_rule: "knock"` should allow knocking ✅
   - Rooms with `join_rule: "knock_restricted"` should allow knocking ✅
   - Rooms with other join rules should reject knocking ✅

---

## Definition of Done

- ✅ `check_room_allows_knocking()` checks both `"knock"` and `"knock_restricted"` join rules
- ✅ Code compiles without errors
- ✅ Implementation matches the pattern used in `room_supports_knocking()` helper
- ✅ No regressions in existing knock functionality

---

## Notes

The implementation is otherwise production-quality:
- Comprehensive X-Matrix authentication
- Room version compatibility checks (v7+)
- Membership state validation
- Server ACL enforcement
- Event structure validation
- Proper error codes (M_FORBIDDEN, M_INCOMPATIBLE_ROOM_VERSION, etc.)
- Complete event signing and verification in send_knock

This is a simple one-line fix that brings the repository method into alignment with the Matrix spec and existing helper functions.
