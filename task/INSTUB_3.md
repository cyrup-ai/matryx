# INSTUB_3: Device List Left Users Tracking

**Priority**: HIGH  
**Estimated Effort**: 1 session  
**Category**: Device List Synchronization

---

## OBJECTIVE

Implement proper "left user" tracking in device list updates to correctly synchronize device keys when users leave rooms.

**WHY**: Device list updates currently don't track when users leave rooms, breaking end-to-end encryption key management. Clients need to know when to remove device keys from their local cache. The `left` array is always empty, violating the Matrix spec.

---

## BACKGROUND

**Current Location**: [`packages/client/src/sync.rs`](../packages/client/src/sync.rs):433

**The Problem**:
```rust
let update = SyncUpdate::DeviceListUpdate {
    changed: vec![device.user_id],
    left: vec![], // TODO: Implement proper left user tracking from membership changes
};
```

**Matrix Spec Requirement**: Device list updates must include:
- `changed`: User IDs whose devices changed
- `left`: User IDs who left rooms since last sync (so client can purge their device keys)

**Repository Available**: `MembershipRepository` already has methods to query users by membership state.

---

## SUBTASK 1: Understand Device List Update Context

**WHAT**: Locate where device list updates are generated and understand the data flow.

**WHERE**: [`packages/client/src/sync.rs`](../packages/client/src/sync.rs)

**FIND**: The function or method that creates `SyncUpdate::DeviceListUpdate`. Likely within:
- A device monitoring loop
- A sync handler
- A real-time subscription processor

**CONTEXT TO UNDERSTAND**:
- What triggers device list updates?
- Is there a "since" timestamp or token for incremental sync?
- What is the data structure of `SyncUpdate::DeviceListUpdate`?

**DEFINITION OF DONE**:
- ✅ Located the function that generates device list updates
- ✅ Understand how to access membership repository in that context
- ✅ Identified if "since" timestamp is available

---

## SUBTASK 2: Query Left and Banned Users

**WHAT**: Use `MembershipRepository` to find users who left or were banned from rooms.

**WHERE**: Same location as SUBTASK 1, within the device list update generation logic

**HOW**: Add membership queries to track state changes:

```rust
// Get users who have left rooms since last sync
let left_memberships = self.membership_repo
    .get_user_rooms_by_state(&self.user_id, "leave")
    .await?;

let banned_memberships = self.membership_repo
    .get_user_rooms_by_state(&self.user_id, "ban")
    .await?;

// Extract user IDs
let mut left_users: Vec<String> = left_memberships
    .into_iter()
    .map(|m| m.user_id)
    .collect();

left_users.extend(
    banned_memberships
        .into_iter()
        .map(|m| m.user_id)
);

// Remove duplicates
left_users.sort();
left_users.dedup();
```

**IMPORTANT**: If there's a "since" timestamp for incremental sync, add filtering:
```rust
// Only include users who left SINCE last sync
let left_since = left_memberships
    .into_iter()
    .filter(|m| {
        m.updated_at
            .map(|ts| ts > since_timestamp)
            .unwrap_or(false)
    })
    .map(|m| m.user_id)
    .collect();
```

**Repository Method Used**:
- `MembershipRepository::get_user_rooms_by_state(user_id, state)` → `Result<Vec<Membership>, RepositoryError>`
- States: `"leave"`, `"ban"`
- Defined in: [`packages/surrealdb/src/repository/membership.rs`](../packages/surrealdb/src/repository/membership.rs):97-113

**DEFINITION OF DONE**:
- ✅ Query retrieves users with "leave" membership state
- ✅ Query retrieves users with "ban" membership state
- ✅ User IDs extracted and deduplicated
- ✅ Filtered by timestamp if incremental sync is used

---

## SUBTASK 3: Populate Left Array in DeviceListUpdate

**WHAT**: Replace the empty `left: vec![]` with the actual left users list.

**WHERE**: [`packages/client/src/sync.rs`](../packages/client/src/sync.rs):433

**CURRENT CODE**:
```rust
let update = SyncUpdate::DeviceListUpdate {
    changed: vec![device.user_id],
    left: vec![], // TODO: Implement proper left user tracking
};
```

**REPLACE WITH**:
```rust
// Get left users (from SUBTASK 2)
let left_users = get_left_users(&self.membership_repo, &self.user_id).await?;

let update = SyncUpdate::DeviceListUpdate {
    changed: vec![device.user_id],
    left: left_users,  // NOW POPULATED
};
```

**OR** (if inline):
```rust
// Query left users
let left_memberships = self.membership_repo
    .get_user_rooms_by_state(&self.user_id, "leave")
    .await?;
let banned_memberships = self.membership_repo
    .get_user_rooms_by_state(&self.user_id, "ban")
    .await?;

let mut left_users: Vec<String> = left_memberships
    .into_iter()
    .map(|m| m.user_id)
    .collect();
left_users.extend(banned_memberships.into_iter().map(|m| m.user_id));
left_users.sort();
left_users.dedup();

let update = SyncUpdate::DeviceListUpdate {
    changed: vec![device.user_id],
    left: left_users,
};
```

**DEFINITION OF DONE**:
- ✅ `left` field populated with actual user IDs
- ✅ Includes both "leave" and "ban" states
- ✅ No duplicate user IDs in the array
- ✅ Empty TODO comment removed

---

## SUBTASK 4: Handle Shared Rooms Edge Case

**WHAT**: Ensure users are only marked as "left" if they don't share ANY rooms with the current user.

**WHY**: If User A and User B are both in Room 1 and Room 2, and User B leaves Room 1, they should NOT be in the "left" list because they still share Room 2.

**WHERE**: Same location, add filtering logic

**HOW**: Add shared room check:

```rust
// Get all users in rooms the current user is still in
let current_rooms = self.membership_repo
    .get_user_rooms(&self.user_id)
    .await?;

let mut users_in_shared_rooms = HashSet::new();
for room in &current_rooms {
    let members = self.membership_repo
        .get_room_members(&room.room_id)
        .await?;
    users_in_shared_rooms.extend(members.into_iter().map(|m| m.user_id));
}

// Filter left_users to only include those with NO shared rooms
let left_users: Vec<String> = left_users
    .into_iter()
    .filter(|user_id| !users_in_shared_rooms.contains(user_id))
    .collect();
```

**PERFORMANCE NOTE**: This may be expensive if user is in many rooms. Consider:
- Caching shared room members
- Only checking if left_users list is non-empty
- Doing this check asynchronously

**DEFINITION OF DONE**:
- ✅ Users not marked as "left" if they share other rooms
- ✅ Only truly disconnected users included
- ✅ Logic handles edge cases correctly

---

## SUBTASK 5: Add Error Handling

**WHAT**: Handle repository errors gracefully.

**WHERE**: Within the device list update logic

**HOW**:
```rust
// If left user tracking fails, log but don't break device list updates
let left_users = match self.get_left_users().await {
    Ok(users) => users,
    Err(e) => {
        tracing::warn!("Failed to get left users for device list: {}", e);
        vec![]  // Fallback to empty, but changed list still works
    }
};

let update = SyncUpdate::DeviceListUpdate {
    changed: vec![device.user_id],
    left: left_users,
};
```

**WHY**: Device key changes (`changed`) are more critical than left users. Don't break the entire update if left tracking fails.

**DEFINITION OF DONE**:
- ✅ Errors logged with context
- ✅ Failures degrade gracefully
- ✅ Device list updates still sent with at least `changed` field

---

## SUBTASK 6: Verify Compilation and Integration

**WHAT**: Ensure code compiles and integrates with sync system.

**WHERE**: Run from workspace root

**HOW**:
```bash
# Build client package
cargo build --package matryx_client

# Check for errors
cargo check --package matryx_client
```

**VERIFY**:
- Check that `MembershipRepository` is available in the sync context
- May need to add it as a field if not already present
- Ensure `SyncUpdate::DeviceListUpdate` struct has `left` field

**DEFINITION OF DONE**:
- ✅ Code compiles without errors
- ✅ All imports resolved
- ✅ Type signatures match

---

## RESEARCH NOTES

### MembershipRepository API
Location: [`packages/surrealdb/src/repository/membership.rs`](../packages/surrealdb/src/repository/membership.rs)

Key methods:
- `get_user_rooms_by_state(user_id, membership_state)` - Get rooms filtered by state
- `get_room_members(room_id)` - Get all members of a room
- `get_user_rooms(user_id)` - Get user's joined rooms

Membership states:
- `"join"` - Active member
- `"leave"` - User left voluntarily
- `"ban"` - User was banned
- `"invite"` - User invited but not joined
- `"knock"` - User requested to join

### Matrix Specification
- **Device List Updates**: Part of `/sync` response
- **Spec Reference**: Client-Server API sync endpoint
- **E2E Encryption**: Device keys must be tracked per spec

Device list format:
```json
{
  "device_lists": {
    "changed": ["@user1:server", "@user2:server"],
    "left": ["@user3:server"]
  }
}
```

### Performance Considerations
- Querying all room members can be expensive
- Consider caching membership data
- May want to implement incremental tracking with timestamps
- Balance correctness vs performance

---

## DEFINITION OF DONE

**Task complete when**:
- ✅ Device list updates include `left` array with actual user IDs
- ✅ Left users include those with "leave" and "ban" membership states
- ✅ Shared room edge case handled correctly
- ✅ Proper error handling prevents update failures
- ✅ Empty TODO comment removed
- ✅ Code compiles successfully
- ✅ Device list synchronization works per Matrix spec

**NO REQUIREMENTS FOR**:
- ❌ Unit tests
- ❌ Integration tests
- ❌ Benchmarks
- ❌ Documentation (beyond code comments)

---

## RELATED FILES

- [`packages/client/src/sync.rs`](../packages/client/src/sync.rs) - Main file to modify (line 433)
- [`packages/surrealdb/src/repository/membership.rs`](../packages/surrealdb/src/repository/membership.rs) - Membership queries
- [`packages/client/src/realtime.rs`](../packages/client/src/realtime.rs) - May contain related sync logic
