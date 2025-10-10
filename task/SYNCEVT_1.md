# SYNCEVT_1: Implement User Events Aggregation in Sync Updates

## OBJECTIVE

Remove the stub at `packages/client/src/repositories/client_service.rs:261` by implementing proper user events aggregation for the sync updates system.

## CONTEXT

**File:** `/Volumes/samsung_t9/maxtryx/packages/client/src/repositories/client_service.rs`  
**Line:** 261  
**Current Code:**
```rust
let events = vec![]; // TODO: Implement user events aggregation when needed
```

The `get_sync_updates()` method currently returns an empty vector for events, which means the sync system is incomplete. This needs to be replaced with actual user event aggregation logic that fetches timeline events from all rooms the user has joined.

## ARCHITECTURE RESEARCH

### Repository Structure

The `ClientRepositoryService` struct (defined starting at line 36 in client_service.rs) contains these repositories:

- `event_repo: EventRepository` - from `matryx_surrealdb::repository`
- `membership_repo: MembershipRepository` - from `matryx_surrealdb::repository`
- `presence_repo: PresenceRepository` - from `matryx_surrealdb::repository`
- `device_repo: DeviceRepository` - from `matryx_surrealdb::repository`
- `to_device_repo: ToDeviceRepository` - from `matryx_surrealdb::repository`
- `user_id: String` - authenticated user identifier
- `device_id: String` - device identifier

Reference: [../../packages/client/src/repositories/client_service.rs](../../packages/client/src/repositories/client_service.rs#L36-L43)

### EventRepository Methods

The `EventRepository` from `matryx_surrealdb::repository` provides:

```rust
pub async fn get_room_timeline(
    &self,
    room_id: &str,
    limit: Option<u32>,
) -> Result<Vec<Event>, RepositoryError>
```

This method fetches timeline events for a specific room, ordered by `origin_server_ts DESC`, with an optional limit.

Reference: [../../packages/surrealdb/src/repository/event.rs](../../packages/surrealdb/src/repository/event.rs#L395-L402)

### Membership Entity Structure

The `Membership` struct from `matryx_entity::types` contains:

```rust
pub struct Membership {
    pub room_id: String,
    pub user_id: String,
    pub membership: MembershipState,
    // ... other fields
}
```

The `MembershipState` enum has these variants:
- `Invite` - User has been invited
- `Join` - User has joined the room (active member)
- `Leave` - User has left
- `Ban` - User has been banned
- `Knock` - User has knocked (requesting to join)

Reference: [../../packages/entity/src/types/membership.rs](../../packages/entity/src/types/membership.rs#L7-L15)  
Reference: [../../packages/entity/src/types/membership_state.rs](../../packages/entity/src/types/membership_state.rs#L6-L15)

### Current Method Implementation Pattern

The `get_sync_updates()` method (lines 258-276) follows this pattern:

```rust
pub async fn get_sync_updates(&self) -> Result<SyncUpdate, ClientError> {
    // 1. Fetch data from each repository
    let events = vec![]; // TODO: Line 261 - NEEDS IMPLEMENTATION
    let membership_changes = self.membership_repo.get_user_rooms(&self.user_id).await?;
    let presence_updates = vec![]; // TODO: Different task
    let device_updates = self.device_repo.get_user_devices(&self.user_id).await?;
    let to_device_messages = self
        .to_device_repo
        .get_to_device_messages(&self.user_id, "", None)
        .await
        .unwrap_or_default();

    // 2. Aggregate into SyncUpdate
    Ok(SyncUpdate {
        events,
        membership_changes,
        presence_updates,
        device_updates,
        to_device_messages,
    })
}
```

**Pattern Observations**:
- Each data type is fetched independently from its repository
- `membership_changes` is already fetched (line 262) containing all rooms the user belongs to
- Events should be aggregated from all JOINED rooms (not invited/left rooms)
- Error handling uses `?` for critical operations, `unwrap_or_default()` for optional features

### SyncUpdate Structure

The return type expects:

```rust
pub struct SyncUpdate {
    pub events: Vec<Event>,
    pub membership_changes: Vec<Membership>,
    pub presence_updates: Vec<UserPresenceUpdate>,
    pub device_updates: Vec<Device>,
    pub to_device_messages: Vec<ToDeviceMessage>,
}
```

Reference: [../../packages/client/src/repositories/client_service.rs](../../packages/client/src/repositories/client_service.rs#L26-L32)

## IMPLEMENTATION SPECIFICATION

### Step 1: Replace Line 261

**Current code (line 261):**
```rust
let events = vec![]; // TODO: Implement user events aggregation when needed
```

**New implementation:**
```rust
// Aggregate events from all joined rooms
let mut events = Vec::new();
for membership in &membership_changes {
    if membership.membership == MembershipState::Join {
        match self.event_repo.get_room_timeline(&membership.room_id, Some(20)).await {
            Ok(room_events) => events.extend(room_events),
            Err(e) => {
                tracing::warn!(
                    "Failed to fetch events for room {}: {}",
                    membership.room_id,
                    e
                );
            }
        }
    }
}
```

### Implementation Details

**Why this approach:**

1. **Reuses existing data**: The `membership_changes` variable (line 262) is already fetched, containing all user room memberships
2. **Filters correctly**: Only `MembershipState::Join` rooms should have events fetched (not invited, left, or banned rooms)
3. **Aggregates efficiently**: Collects events from all joined rooms into a single vector
4. **Limits per-room**: Uses `Some(20)` to fetch 20 most recent events per room, preventing excessive data transfer
5. **Resilient error handling**: Logs failures but continues processing other rooms (one failing room doesn't break entire sync)
6. **Follows patterns**: Matches the error handling and data fetching patterns used elsewhere in the method

**Why limit to 20 events per room:**
- Sync updates should be lightweight snapshots, not full history
- Matches the limit used in `SyncRepository::get_room_timeline_events` (line 801 in sync.rs)
- Can be adjusted based on performance requirements without changing the logic

**Error handling strategy:**
- Uses `match` to handle `Result<Vec<Event>, RepositoryError>`
- On success: Extends the events vector with room events
- On error: Logs warning with room_id and error details, continues processing
- This prevents one failing room from breaking the entire sync update

### Required Import

Ensure `MembershipState` is imported. Check line 2 of client_service.rs:

```rust
use matryx_entity::{Device, Event, Membership, UserPresenceUpdate};
```

If `MembershipState` is not included, add it:

```rust
use matryx_entity::{Device, Event, Membership, MembershipState, UserPresenceUpdate};
```

However, reviewing the imports in the file shows that `MembershipState` should already be accessible through the `matryx_entity` crate since `Membership` is imported.

### Step 2: Remove TODO Comment

Delete the TODO comment completely. The implementation should have no placeholder comments.

## EXECUTION CHECKLIST

1. Open `/Volumes/samsung_t9/maxtryx/packages/client/src/repositories/client_service.rs`
2. Navigate to line 261
3. Replace `let events = vec![]; // TODO: Implement user events aggregation when needed` with the implementation specified above
4. Verify `MembershipState` is in scope (should be via `matryx_entity::types`)
5. Verify code compiles: `cargo check -p matryx_client`
6. Verify no TODO comments remain in the method

## DEFINITION OF DONE

- [ ] Line 261 stub `let events = vec![];` is replaced with event aggregation logic
- [ ] Events are fetched from `EventRepository` using `get_room_timeline()`
- [ ] Only `MembershipState::Join` rooms have events fetched
- [ ] Events from all joined rooms are aggregated into a single `Vec<Event>`
- [ ] Error handling logs warnings but doesn't fail entire sync
- [ ] TODO comment is completely removed
- [ ] Code compiles without errors: `cargo check -p matryx_client`
- [ ] No unwrap() or expect() calls are used in the implementation
- [ ] Implementation follows existing patterns in the same method

## REFERENCES

### Source Files Analyzed

- [`packages/client/src/repositories/client_service.rs`](../../packages/client/src/repositories/client_service.rs) - Target file
- [`packages/surrealdb/src/repository/event.rs`](../../packages/surrealdb/src/repository/event.rs) - EventRepository methods
- [`packages/surrealdb/src/repository/sync.rs`](../../packages/surrealdb/src/repository/sync.rs) - Sync patterns and timeline limits
- [`packages/entity/src/types/membership.rs`](../../packages/entity/src/types/membership.rs) - Membership entity
- [`packages/entity/src/types/membership_state.rs`](../../packages/entity/src/types/membership_state.rs) - MembershipState enum
- [`packages/client/src/sync.rs`](../../packages/client/src/sync.rs) - Sync architecture patterns

### Key Findings

1. **EventRepository** is already available in the struct as `self.event_repo`
2. **get_room_timeline()** is the appropriate method (not get_room_events which is lower-level)
3. **membership_changes** is already fetched on line 262, should be reused
4. **Limit of 20 events per room** matches SurrealDB SyncRepository implementation
5. **Resilient error handling** is the pattern used elsewhere (e.g., to_device_messages with unwrap_or_default)

## CONSTRAINTS

- **DO NOT** write any code changes beyond line 261 replacement
- **DO NOT** modify method signatures
- **DO NOT** add new dependencies
- **DO NOT** use unwrap() or expect() - use proper error handling
- **ONLY** modify production source code in `packages/client/src/repositories/client_service.rs`
