# DEVLEFT_1: Implement Left User Tracking for Device List Updates

## OBJECTIVE

Remove the stub at `packages/client/src/sync.rs:433` by implementing proper tracking of users who have left rooms, so device list updates include the complete `left` field as required by the Matrix specification.

## CONTEXT

**File:** `/Volumes/samsung_t9/maxtryx/packages/client/src/sync.rs`  
**Line:** 433  
**Current Code:**
```rust
let update = SyncUpdate::DeviceListUpdate {
    changed: vec![device.user_id],
    left: vec![], // TODO: Implement proper left user tracking from membership changes
};
```

### What This Feature Does

In Matrix's end-to-end encryption system, clients must track when users leave encrypted rooms to properly manage encryption sessions. The `device_lists` structure in sync responses contains two fields:

- **`changed`**: User IDs whose device lists have changed (new devices, removed devices, device key updates)
- **`left`**: User IDs who have left encrypted rooms that the current user is in

When a user leaves a room, the client should:
1. Invalidate any active outbound Megolm encryption sessions for that room
2. Stop including that user in future encrypted messages for that room
3. Remove their devices from the trusted device list for that room context

**Reference**: [Matrix Client-Server API - Extensions to Sync](https://spec.matrix.org/v1.11/client-server-api/#extensions-to-sync)

## ARCHITECTURE ANALYSIS

### Current LiveQuery Implementation

The `LiveQuerySync` struct manages real-time Matrix synchronization using SurrealDB LiveQuery subscriptions. It runs multiple independent async tasks that subscribe to different types of updates:

1. **`start_event_subscriptions()`** - Subscribes to room events (line 253)
2. **`start_membership_subscriptions()`** - Subscribes to membership changes (line 309)
3. **`start_presence_subscriptions()`** - Subscribes to presence updates (line 351)
4. **`start_device_subscriptions()`** - Subscribes to device updates (line 393)

**The Problem**: These subscriptions run in isolated `tokio::spawn` tasks and don't share state about membership changes. When a device update notification arrives in `start_device_subscriptions()`, there's no way to know which users have recently left rooms.

### Data Flow

```
SurrealDB LiveQuery (membership) → start_membership_subscriptions() → MembershipUpdate event
                                          ↓
                                    (NO COORDINATION)
                                          ↓
SurrealDB LiveQuery (devices) → start_device_subscriptions() → DeviceListUpdate event
                                     ↑
                                  LINE 433: left: vec![] ← NEEDS DATA
```

### Existing Data Structures

**`SyncState`** (line 23-38) - Already tracks rooms:
```rust
pub struct SyncState {
    pub next_batch: String,
    pub joined_rooms: HashMap<String, JoinedRoomState>,
    pub invited_rooms: HashMap<String, InvitedRoomState>,
    pub left_rooms: HashMap<String, LeftRoomState>,  // ← Tracks left rooms but not efficiently
    // ...
}
```

**`Membership`** (from `matryx_entity`, imported line 11):
```rust
pub struct Membership {
    pub room_id: String,
    pub user_id: String,
    pub membership: MembershipState,  // Join, Leave, Invite, Ban, Knock
    // ... other fields
}
```

**Reference**: See [../packages/entity/src/types/membership.rs](../packages/entity/src/types/membership.rs) and [../packages/entity/src/types/membership_state.rs](../packages/entity/src/types/membership_state.rs)

### Repository Services Available

The `ClientRepositoryService` (see [./repositories/client_service.rs](../packages/client/src/repositories/client_service.rs)) provides:

- `subscribe_to_membership_changes()` - Already used in line 319
- `subscribe_to_device_updates()` - Already used in line 407
- `get_user_memberships()` - Could query current state (but expensive to call repeatedly)

## IMPLEMENTATION PLAN

### Overview

Add shared state to `LiveQuerySync` that tracks users who have left rooms. The membership subscription task will populate this state, and the device subscription task will read and drain it.

### Step 1: Add Import for HashSet

**Location**: Top of file after existing imports (around line 17)

**Add**:
```rust
use std::collections::HashSet;
```

### Step 2: Add Field to LiveQuerySync Struct

**Location**: `LiveQuerySync` struct definition (around line 169)

**Current**:
```rust
pub struct LiveQuerySync {
    user_id: String,
    state: Arc<RwLock<SyncState>>,
    repository_service: ClientRepositoryService,
    update_sender: broadcast::Sender<SyncUpdate>,
    update_receiver: broadcast::Receiver<SyncUpdate>,
}
```

**Change to**:
```rust
pub struct LiveQuerySync {
    user_id: String,
    state: Arc<RwLock<SyncState>>,
    repository_service: ClientRepositoryService,
    update_sender: broadcast::Sender<SyncUpdate>,
    update_receiver: broadcast::Receiver<SyncUpdate>,
    /// Track users who have left rooms for device list updates
    /// This set is populated by membership subscriptions and consumed by device subscriptions
    left_users: Arc<RwLock<HashSet<String>>>,
}
```

### Step 3: Initialize Field in Constructor

**Location**: `LiveQuerySync::new()` method (around line 179)

**Current**:
```rust
Self {
    user_id,
    state: Arc::new(RwLock::new(SyncState::default())),
    repository_service,
    update_sender,
    update_receiver,
}
```

**Change to**:
```rust
Self {
    user_id,
    state: Arc::new(RwLock::new(SyncState::default())),
    repository_service,
    update_sender,
    update_receiver,
    left_users: Arc::new(RwLock::new(HashSet::new())),
}
```

### Step 4: Track Left Users in Membership Subscription

**Location**: `start_membership_subscriptions()` method, inside the tokio::spawn task (around line 324-342)

**Current**:
```rust
while let Some(notification_result) = stream.next().await {
    match notification_result {
        Ok(memberships) => {
            for membership in memberships {
                debug!(
                    "Received membership update for user {} in room {}: {}",
                    user_id_clone, membership.room_id, membership.membership
                );

                let room_id_owned = membership.room_id.clone();
                let update = SyncUpdate::MembershipUpdate {
                    room_id: room_id_owned,
                    user_id: user_id_clone.clone(),
                    membership,
                };

                if let Err(e) = update_sender_clone.send(update) {
                    warn!("Failed to send membership update: {}", e);
                }
            }
        },
```

**Change to**:
```rust
while let Some(notification_result) = stream.next().await {
    match notification_result {
        Ok(memberships) => {
            for membership in memberships {
                debug!(
                    "Received membership update for user {} in room {}: {}",
                    user_id_clone, membership.room_id, membership.membership
                );

                // Track users who left for device list updates
                if membership.membership == MembershipState::Leave {
                    let mut left_users = left_users_clone.write().await;
                    left_users.insert(membership.user_id.clone());
                    debug!("Added user {} to left users tracking", membership.user_id);
                }

                let room_id_owned = membership.room_id.clone();
                let update = SyncUpdate::MembershipUpdate {
                    room_id: room_id_owned,
                    user_id: user_id_clone.clone(),
                    membership,
                };

                if let Err(e) = update_sender_clone.send(update) {
                    warn!("Failed to send membership update: {}", e);
                }
            }
        },
```

**Note**: Need to clone `left_users` Arc before the tokio::spawn. Add this before spawning:

```rust
let left_users_clone = self.left_users.clone();
```

### Step 5: Populate Left Field in Device Subscription

**Location**: `start_device_subscriptions()` method, inside the tokio::spawn task (around line 427-435)

**Current**:
```rust
Ok(notification) => {
    // notification is now Device directly, not DeviceKeys
    let device = notification;
    let update = SyncUpdate::DeviceListUpdate {
        changed: vec![device.user_id],
        left: vec![], // TODO: Implement proper left user tracking from membership changes
    };

    if let Err(e) = update_sender_clone.send(update) {
        warn!("Failed to send device list update: {}", e);
    }
},
```

**Change to**:
```rust
Ok(notification) => {
    // notification is now Device directly, not DeviceKeys
    let device = notification;
    
    // Get and clear left users atomically
    // This ensures each user appears in 'left' exactly once after they leave
    let left = {
        let mut left_users = left_users_clone.write().await;
        let users: Vec<String> = left_users.iter().cloned().collect();
        left_users.clear();
        users
    };
    
    if !left.is_empty() {
        debug!("Including {} left users in device list update", left.len());
    }
    
    let update = SyncUpdate::DeviceListUpdate {
        changed: vec![device.user_id],
        left,
    };

    if let Err(e) = update_sender_clone.send(update) {
        warn!("Failed to send device list update: {}", e);
    }
},
```

**Note**: Need to clone `left_users` Arc before the first tokio::spawn in this method. Add this before spawning:

```rust
let left_users_clone = self.left_users.clone();
```

And also clone it again for the second tokio::spawn in the same method (for to-device messages):

```rust
let left_users_clone_2 = self.left_users.clone();
```

## CODE PATTERN REFERENCE

### Reference Implementation

The Matrix Rust SDK handles this with a `DeviceLists` struct that tracks both changed and left users:

**File**: [../tmp/matrix-rust-sdk/crates/matrix-sdk-crypto/src/machine/mod.rs](../tmp/matrix-rust-sdk/crates/matrix-sdk-crypto/src/machine/mod.rs) (line 3067)

```rust
pub struct EncryptionSyncChanges<'a> {
    pub to_device_events: Vec<Raw<AnyToDeviceEvent>>,
    /// The mapping of changed and left devices, per user, as returned in the
    /// sync response.
    pub changed_devices: &'a DeviceLists,
    // ...
}
```

### Concurrency Pattern

The "drain on read" pattern used in Step 5 ensures:
1. **Thread safety**: Write lock prevents concurrent access
2. **Atomicity**: Clone and clear happen together
3. **No duplicates**: Each user appears in `left` exactly once
4. **No memory leaks**: Set is cleared after reading

## TECHNICAL CONSIDERATIONS

### Why HashSet?

- **O(1) insertion**: Fast when membership updates arrive
- **O(1) lookup**: Efficient deduplication if same user leaves multiple rooms
- **Memory efficient**: Only stores unique user IDs
- **Thread-safe**: Wrapped in `Arc<RwLock<>>` for multi-threaded access

### Why "Drain on Read"?

Matrix sync responses represent changes "since last sync". In our LiveQuery implementation, each `DeviceListUpdate` event is analogous to a sync response. The drain pattern ensures users appear in `left` exactly once per membership change, matching expected Matrix semantics.

### Alternative Approaches Considered

1. **Query database on every device update**: Too expensive, creates coupling
2. **Use SyncState.left_rooms**: Not indexed by user ID, requires traversal
3. **Time-based expiry**: More complex, risk of reporting users multiple times
4. **Merge streams**: Overly complex for this use case

## FILES TO MODIFY

All changes are in a single file:

- [`/Volumes/samsung_t9/maxtryx/packages/client/src/sync.rs`](../packages/client/src/sync.rs)

## IMPORTS NEEDED

Add to existing import block:
```rust
use std::collections::HashSet;
```

## DEFINITION OF DONE

- [ ] `HashSet` import added to sync.rs
- [ ] `left_users: Arc<RwLock<HashSet<String>>>` field added to `LiveQuerySync` struct
- [ ] Field initialized in `new()` constructor
- [ ] `left_users` cloned before tokio::spawn in `start_membership_subscriptions()`
- [ ] Left user tracking logic added when `MembershipState::Leave` detected
- [ ] `left_users` cloned before both tokio::spawn calls in `start_device_subscriptions()`
- [ ] "Drain on read" logic implemented to populate `left` field
- [ ] TODO comment at line 433 completely removed
- [ ] Empty `vec![]` stub replaced with actual left user data
- [ ] Code compiles without errors or warnings
- [ ] No `unwrap()` or `expect()` calls used (all error handling is safe)
- [ ] Debug logging added for observability

## CONSTRAINTS

- **DO NOT** write any test code, benchmark code, or documentation
- **DO NOT** modify any files outside of `packages/client/src/sync.rs`  
- **DO NOT** change the public API or struct field visibility
- **DO NOT** use `unwrap()` or `expect()` - all error handling must be production-safe
- **ONLY** modify production source code to implement the feature as specified
