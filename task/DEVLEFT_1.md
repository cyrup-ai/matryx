# DEVLEFT_1: Fix Compilation Error in Left User Tracking

## CRITICAL ISSUE

**File:** `/Volumes/samsung_t9/maxtryx/packages/client/src/sync.rs`  
**Lines:** 452-487

### Compilation Error: Use-After-Move

The implementation has a critical Rust ownership error that prevents compilation:

```
error[E0382]: use of moved value: `preliminary_left_users`
   --> packages/client/src/sync.rs:483:45
```

**Location of Bug:**

```rust
// Line 452-456: preliminary_left_users is created
let preliminary_left_users = {
    let mut left_users = left_users_clone.write().await;
    let users: Vec<String> = left_users.iter().cloned().collect();
    left_users.clear();
    users
};

// Line 465: preliminary_left_users is MOVED here
match Self::filter_truly_left_users(
    &repository_service,
    &user_id_clone,
    preliminary_left_users  // <-- MOVED (ownership transferred)
).await {
    Ok(filtered_users) => {
        // ...
        filtered_users
    },
    Err(e) => {
        warn!("Failed to filter left users...");
        preliminary_left_users  // <-- ERROR: Used after move!
    }
}
```

### Required Fix

**Option 1 (Recommended):** Clone before passing to filter function
```rust
let preliminary_left_users = {
    let mut left_users = left_users_clone.write().await;
    let users: Vec<String> = left_users.iter().cloned().collect();
    left_users.clear();
    users
};

let left = if !preliminary_left_users.is_empty() {
    match Self::filter_truly_left_users(
        &repository_service,
        &user_id_clone,
        preliminary_left_users.clone()  // <-- Clone here
    ).await {
        Ok(filtered_users) => {
            if !filtered_users.is_empty() {
                debug!(
                    "Including {} truly left users in device list update",
                    filtered_users.len()
                );
            }
            filtered_users
        },
        Err(e) => {
            warn!(
                "Failed to filter left users (shared rooms check): {}. \
                Falling back to unfiltered list.",
                e
            );
            preliminary_left_users  // <-- Now valid
        }
    }
} else {
    Vec::new()
};
```

**Option 2 (More Efficient):** Change filter function signature to take reference
```rust
// Change line 677 from:
candidate_left_users: Vec<String>,

// To:
candidate_left_users: &[String],

// Then update the filter logic at line 725 to work with references
```

## COMPLETED ITEMS

The following have been correctly implemented:
- ✅ `HashSet` import added 
- ✅ `left_users: Arc<RwLock<HashSet<String>>>` field added to struct
- ✅ Field initialized in constructor
- ✅ `left_users` cloned before tokio::spawn in membership subscription
- ✅ Left user tracking logic implemented for Leave and Ban states
- ✅ `left_users` cloned before tokio::spawn in device subscription
- ✅ Drain-on-read pattern implemented
- ✅ TODO comment removed at line 433
- ✅ Debug logging added

## ADDITIONAL OBSERVATIONS

### Extra Complexity Added (Beyond Requirements)

The implementation includes a `filter_truly_left_users` method (lines 674-755) that handles the "shared rooms edge case." This is technically correct per Matrix specification but was NOT required by the original task.

**Considerations:**
- The shared rooms filtering performs multiple database queries per device update
- This may have performance implications at scale
- The basic implementation (just tracking leave/ban) would have satisfied the task requirements
- The added complexity introduced the compilation bug

### Recommendation

For this task completion:
1. **MUST FIX:** The compilation error (use Option 1 above with .clone())
2. **OPTIONAL:** Keep the advanced filtering logic as-is since it's architecturally sound
3. **VERIFY:** Code compiles with `cargo build -p matryx_client`

## DEFINITION OF DONE

- [ ] Compilation error fixed (preliminary_left_users ownership issue)
- [ ] Code compiles without errors: `cargo build -p matryx_client`
- [ ] No new warnings introduced
- [ ] Drain-on-read pattern remains intact
- [ ] Error handling remains production-safe (no panics)

## FILES TO MODIFY

- `/Volumes/samsung_t9/maxtryx/packages/client/src/sync.rs` (lines 452-487)

## CONSTRAINTS

- **DO NOT** remove the filter_truly_left_users method (it's correct, just needs the caller fixed)
- **DO NOT** change error handling to use unwrap() or expect()
- **ONLY** fix the ownership/move issue to make code compile
