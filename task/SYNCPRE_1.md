# SYNCPRE_1: Implement Presence Updates Integration

## OBJECTIVE

Remove the stub at `packages/client/src/repositories/client_service.rs:263` by implementing proper presence updates integration for the sync system.

## CONTEXT

**File:** `/Volumes/samsung_t9/maxtryx/packages/client/src/repositories/client_service.rs`  
**Line:** 263  
**Current Code:**
```rust
let presence_updates = vec![]; // TODO: Implement when presence repository has the method
```

The `get_sync_updates()` method currently returns an empty vector for presence updates. This needs to be replaced with actual presence data from the presence repository.

## CODE ANALYSIS

### Key Discovery: Everything Already Exists!

**ClientRepositoryService struct** (lines 35-45):
```rust
pub struct ClientRepositoryService {
    event_repo: EventRepository,
    membership_repo: MembershipRepository,
    presence_repo: PresenceRepository,  // ← ALREADY EXISTS!
    device_repo: DeviceRepository,
    to_device_repo: ToDeviceRepository,
    user_id: String,
    device_id: String,
}
```

**SyncUpdate struct** (lines 27-33):
```rust
pub struct SyncUpdate {
    pub events: Vec<Event>,
    pub membership_changes: Vec<Membership>,
    pub presence_updates: Vec<UserPresenceUpdate>,  // ← Type is UserPresenceUpdate
    pub device_updates: Vec<Device>,
    pub to_device_messages: Vec<ToDeviceMessage>,
}
```

**Existing presence method** (lines 97-102):
```rust
pub async fn get_user_presence(
    &self,
    user_id: &str,
) -> Result<Option<UserPresenceUpdate>, ClientError> {
    let presence = self.presence_repo.get_user_presence(user_id).await?;
    Ok(presence)
}
```

### PresenceRepository Methods Available

See [../../packages/surrealdb/src/repository/presence.rs](../../packages/surrealdb/src/repository/presence.rs)

**Key methods:**
- `get_user_presence(user_id)` → `Option<UserPresenceUpdate>` (single user)
- `get_multiple_user_presence(user_ids: &[String])` → `Vec<UserPresenceUpdate>` (multiple users) ✓ **USE THIS**
- `subscribe_to_user_presence(user_id)` → Stream (real-time updates)

### MembershipRepository Methods Available

See [../../packages/surrealdb/src/repository/membership.rs](../../packages/surrealdb/src/repository/membership.rs)

**Key method for getting room members:**
- `get_room_members(room_id)` → `Vec<Membership>` (line 75-85)

### Matrix Specification Reference

From Synapse reference implementation ([../../tmp/synapse/synapse/handlers/sync.py](../../tmp/synapse/synapse/handlers/sync.py):1844-1896):

Presence in sync responses includes:
1. Users in rooms the syncing user has joined
2. Newly joined or invited users
3. Deduplicated results (one presence per user)

Implementation pattern from Synapse:
```python
# Get presence for newly joined users and rooms
extra_users_ids = set(newly_joined_or_invited_users)
for room_id in newly_joined_rooms:
    users = await self.store.get_users_in_room(room_id)
    extra_users_ids.update(users)
extra_users_ids.discard(user.to_string())  # Remove self

if extra_users_ids:
    states = await self.presence_handler.get_states(extra_users_ids)
    presence.extend(states)
    presence = list({p.user_id: p for p in presence}.values())  # Deduplicate
```

## IMPLEMENTATION APPROACHES

### Approach 1: Simple MVP (Current User Only)

**Fastest to implement, minimal overhead:**

```rust
let presence_updates = match self.presence_repo.get_user_presence(&self.user_id).await? {
    Some(presence) => vec![presence],
    None => vec![],
};
```

**Pros:** 
- Simple, fast, minimal database queries
- Good for initial implementation

**Cons:**
- Doesn't fully match Matrix spec (should include users in shared rooms)
- Limited functionality

### Approach 2: Comprehensive (Users in Shared Rooms)

**Matches Matrix specification, full functionality:**

```rust
// Extract room IDs from membership_changes
let room_ids: Vec<String> = membership_changes.iter()
    .map(|m| m.room_id.clone())
    .collect();

// Collect all user IDs from rooms the user is in
let mut user_ids = std::collections::HashSet::new();
for room_id in &room_ids {
    let members = self.membership_repo.get_room_members(room_id).await?;
    for member in members {
        user_ids.insert(member.user_id);
    }
}

// Remove self from the list
user_ids.remove(&self.user_id);

// Fetch presence for all users
let user_ids_vec: Vec<String> = user_ids.into_iter().collect();
let presence_updates = if !user_ids_vec.is_empty() {
    self.presence_repo.get_multiple_user_presence(&user_ids_vec).await?
} else {
    vec![]
};
```

**Pros:**
- Fully Matrix-spec compliant
- Provides presence for all visible users
- Uses existing repository methods

**Cons:**
- More database queries (one per room)
- Higher overhead for users in many rooms

### Approach 3: Optimized (Parallel Fetching)

**Best performance for users in multiple rooms:**

```rust
use futures::future::join_all;

// Extract room IDs
let room_ids: Vec<String> = membership_changes.iter()
    .map(|m| m.room_id.clone())
    .collect();

// Fetch all room members in parallel
let member_futures: Vec<_> = room_ids.iter()
    .map(|room_id| self.membership_repo.get_room_members(room_id))
    .collect();

let member_results = join_all(member_futures).await;

// Collect unique user IDs
let mut user_ids = std::collections::HashSet::new();
for result in member_results {
    if let Ok(members) = result {
        for member in members {
            user_ids.insert(member.user_id);
        }
    }
}

// Remove self
user_ids.remove(&self.user_id);

// Fetch presence
let user_ids_vec: Vec<String> = user_ids.into_iter().collect();
let presence_updates = if !user_ids_vec.is_empty() {
    self.presence_repo.get_multiple_user_presence(&user_ids_vec).await?
} else {
    vec![]
};
```

## RECOMMENDED IMPLEMENTATION

**Start with Approach 2 (Comprehensive)** for the following reasons:
1. All required methods already exist in the codebase
2. Matches Matrix specification behavior  
3. Straightforward sequential logic
4. Can optimize later if needed

## EXACT CHANGES REQUIRED

**File:** `packages/client/src/repositories/client_service.rs`  
**Location:** Line 263 in the `get_sync_updates()` method

**Replace this:**
```rust
let presence_updates = vec![]; // TODO: Implement when presence repository has the method
```

**With this:**
```rust
// Extract room IDs from membership_changes
let room_ids: Vec<String> = membership_changes.iter()
    .map(|m| m.room_id.clone())
    .collect();

// Collect all user IDs from rooms the user is in
let mut user_ids = std::collections::HashSet::new();
for room_id in &room_ids {
    let members = self.membership_repo.get_room_members(room_id).await?;
    for member in members {
        user_ids.insert(member.user_id);
    }
}

// Remove self from the list
user_ids.remove(&self.user_id);

// Fetch presence for all users in shared rooms
let user_ids_vec: Vec<String> = user_ids.into_iter().collect();
let presence_updates = if !user_ids_vec.is_empty() {
    self.presence_repo.get_multiple_user_presence(&user_ids_vec).await?
} else {
    vec![]
};
```

**Required imports:** Already present in file:
- `std::collections::HashSet` may need to be added to imports at top of file

## DEFINITION OF DONE

- [ ] Empty `vec![]` stub is removed from line 263
- [ ] Actual presence updates fetching is implemented using `get_multiple_user_presence()`
- [ ] Presence data is fetched for users in shared rooms with the syncing user
- [ ] TODO comment is completely removed
- [ ] Current user (self.user_id) is excluded from presence results
- [ ] Code compiles without errors or warnings
- [ ] Error handling uses `?` operator (no unwrap() or expect())
- [ ] Implementation matches one of the documented approaches above

## CONSTRAINTS

- **DO NOT** write any test code
- **DO NOT** write any benchmark code  
- **DO NOT** add documentation comments beyond basic inline explanations
- **ONLY** modify the production source code at line 263 in `packages/client/src/repositories/client_service.rs`
- **USE** existing repository methods - no need to add new methods to PresenceRepository
- **DO NOT** use unwrap() or expect() - use `?` for error propagation

## NOTES

- The TODO comment incorrectly states "when presence repository has the method" - the method (`get_multiple_user_presence`) **already exists**
- All required infrastructure is already in place - this is purely removing a stub and calling existing methods
- For large rooms, this may fetch presence for many users - performance optimization can be addressed in a separate task if needed
- The implementation follows the same pattern used by Synapse (the reference Matrix homeserver implementation)
