# INSTUB_2: Client Sync Events and Presence Aggregation

**Priority**: HIGH  
**Estimated Effort**: 1 session  
**Category**: Client Sync Implementation

---

## OBJECTIVE

Implement user event aggregation and presence updates in the client sync system by wiring up existing repository methods to replace empty vector placeholders.

**WHY**: The sync endpoint currently returns empty arrays for events and presence, making the sync system non-functional. Users cannot see room events or presence updates. The infrastructure already exists - it just needs to be connected.

---

## BACKGROUND

**Current Location**: [`packages/client/src/repositories/client_service.rs`](../packages/client/src/repositories/client_service.rs):261-263

**The Problem**:
```rust
pub async fn get_sync_updates(&self) -> Result<SyncUpdate, ClientError> {
    let events = vec![]; // TODO: Implement user events aggregation when needed
    let membership_changes = self.membership_repo.get_user_rooms(&self.user_id).await?;
    let presence_updates = vec![]; // TODO: Implement when presence repository has the method
    // ...
}
```

**CRITICAL DISCOVERY**: The TODO comments are **WRONG**! The repositories already have all required methods:
- ✅ `EventRepository` has event aggregation methods
- ✅ `PresenceRepository` has presence tracking methods

We just need to wire them up.

**Matrix Spec Requirement**: `/sync` endpoint MUST return timeline events and presence updates per Client-Server API.

---

## SUBTASK 1: Add Repository Fields to ClientService

**WHAT**: Add `EventRepository` and `PresenceRepository` to the `ClientService` struct.

**WHERE**: [`packages/client/src/repositories/client_service.rs`](../packages/client/src/repositories/client_service.rs)

**CURRENT STRUCT** (approximately line 15-25):
```rust
pub struct ClientService {
    db: Surreal<Any>,
    user_id: String,
    membership_repo: MembershipRepository,
    device_repo: DeviceRepository,
    to_device_repo: ToDeviceRepository,
    // ... other fields
}
```

**ADD THESE FIELDS**:
```rust
pub struct ClientService {
    db: Surreal<Any>,
    user_id: String,
    membership_repo: MembershipRepository,
    device_repo: DeviceRepository,
    to_device_repo: ToDeviceRepository,
    event_repo: EventRepository,      // ADD THIS
    presence_repo: PresenceRepository, // ADD THIS
    // ... other fields
}
```

**UPDATE CONSTRUCTOR**: Add to `new()` or `with_repositories()` method:
```rust
impl ClientService {
    pub fn new(db: Surreal<Any>, user_id: String) -> Self {
        Self {
            db: db.clone(),
            user_id,
            membership_repo: MembershipRepository::new(db.clone()),
            device_repo: DeviceRepository::new(db.clone()),
            to_device_repo: ToDeviceRepository::new(db.clone()),
            event_repo: EventRepository::new(db.clone()),      // ADD
            presence_repo: PresenceRepository::new(db.clone()), // ADD
            // ... other fields
        }
    }
}
```

**IMPORTS NEEDED**:
```rust
use crate::repository::event::EventRepository;
use crate::repository::presence::PresenceRepository;
```

**DEFINITION OF DONE**:
- ✅ EventRepository field added to ClientService struct
- ✅ PresenceRepository field added to ClientService struct
- ✅ Both repositories initialized in constructor
- ✅ Proper imports added
- ✅ Code compiles without errors

---

## SUBTASK 2: Implement User Events Aggregation

**WHAT**: Replace empty events vector with actual event aggregation from user's rooms.

**WHERE**: [`packages/client/src/repositories/client_service.rs`](../packages/client/src/repositories/client_service.rs):261

**CURRENT CODE**:
```rust
let events = vec![]; // TODO: Implement user events aggregation when needed
```

**REPLACE WITH**:
```rust
// Get user's joined rooms
let user_rooms = self.membership_repo.get_user_rooms(&self.user_id).await?;

// Aggregate events from all rooms (last 50 per room)
let mut events = Vec::new();
for membership in &user_rooms {
    let room_events = self.event_repo
        .get_room_events_since(
            &membership.room_id,
            None,        // since_ts: None = get recent events
            Some(50),    // limit: 50 events per room
        )
        .await?;
    
    events.extend(room_events);
}

// Sort by timestamp (most recent first)
events.sort_by(|a, b| b.origin_server_ts.cmp(&a.origin_server_ts));
```

**ALTERNATIVE** (for real-time subscriptions):
If the code uses subscriptions instead of polling, you can use:
```rust
let event_stream = self.event_repo
    .subscribe_user_events(&self.user_id)
    .await?;
```

**Repository Method Used**:
- `EventRepository::get_room_events_since(room_id, since_ts, limit)` → `Result<Vec<Event>, RepositoryError>`
- Defined in: [`packages/surrealdb/src/repository/event.rs`](../packages/surrealdb/src/repository/event.rs):120-150

**DEFINITION OF DONE**:
- ✅ Events vector populated with actual room events
- ✅ Events aggregated from all user's joined rooms
- ✅ Events sorted by timestamp
- ✅ Sync response includes timeline events

---

## SUBTASK 3: Implement Presence Updates Aggregation

**WHAT**: Replace empty presence vector with actual presence data from users in joined rooms.

**WHERE**: [`packages/client/src/repositories/client_service.rs`](../packages/client/src/repositories/client_service.rs):263

**CURRENT CODE**:
```rust
let presence_updates = vec![]; // TODO: Implement when presence repository has the method
```

**REPLACE WITH**:
```rust
// Get all users from rooms the current user is in
let user_rooms = self.membership_repo.get_user_rooms(&self.user_id).await?;
let mut all_user_ids = Vec::new();

for membership in &user_rooms {
    let room_members = self.membership_repo
        .get_room_members(&membership.room_id)
        .await?;
    
    all_user_ids.extend(room_members.into_iter().map(|m| m.user_id));
}

// Remove duplicates
all_user_ids.sort();
all_user_ids.dedup();

// Get presence for all users
let presence_events = self.presence_repo
    .get_presence_events_for_users(&all_user_ids, None)
    .await?;

// Convert to UserPresenceUpdate format
let presence_updates: Vec<UserPresenceUpdate> = presence_events
    .into_iter()
    .map(|pe| UserPresenceUpdate {
        user_id: pe.user_id,
        presence: pe.presence,
        last_active_ago: pe.last_active_ago,
        status_msg: pe.status_msg,
        currently_active: Some(pe.currently_active),
    })
    .collect();
```

**Repository Method Used**:
- `PresenceRepository::get_presence_events_for_users(user_ids, since)` → `Result<Vec<PresenceEvent>, RepositoryError>`
- Defined in: [`packages/surrealdb/src/repository/presence.rs`](../packages/surrealdb/src/repository/presence.rs):155-185

**DEFINITION OF DONE**:
- ✅ Presence updates populated with actual presence data
- ✅ Presence gathered for all users in joined rooms
- ✅ Duplicates removed from user list
- ✅ Sync response includes presence updates conforming to Matrix spec

---

## SUBTASK 4: Handle Error Cases

**WHAT**: Ensure proper error handling for repository calls.

**WHERE**: Same file, within `get_sync_updates()` method

**ERROR HANDLING PATTERN**:
```rust
// If event retrieval fails for one room, log but continue
let mut events = Vec::new();
for membership in &user_rooms {
    match self.event_repo.get_room_events_since(&membership.room_id, None, Some(50)).await {
        Ok(room_events) => events.extend(room_events),
        Err(e) => {
            tracing::warn!(
                "Failed to get events for room {}: {}",
                membership.room_id,
                e
            );
            // Continue with other rooms
        }
    }
}
```

**WHY**: If one room fails to load, the entire sync shouldn't fail. Degrade gracefully.

**DEFINITION OF DONE**:
- ✅ Individual room failures don't break entire sync
- ✅ Errors are logged with context
- ✅ Sync returns partial results when possible

---

## SUBTASK 5: Verify Integration

**WHAT**: Ensure the modified code compiles and integrates with the rest of the client.

**WHERE**: Run from workspace root

**HOW**:
```bash
# Build the client package
cargo build --package matryx_client

# Check for compilation errors
cargo check --package matryx_client
```

**VERIFY**: Look at the `SyncUpdate` return type to ensure it matches what we're populating:
- Check if `SyncUpdate` struct has `events` and `presence_updates` fields
- Ensure field types match what we're returning

**DEFINITION OF DONE**:
- ✅ Code compiles without errors
- ✅ No type mismatches
- ✅ All imports resolve correctly

---

## RESEARCH NOTES

### EventRepository API
Location: [`packages/surrealdb/src/repository/event.rs`](../packages/surrealdb/src/repository/event.rs)

Key methods:
- `get_room_events_since(room_id, since_ts, limit)` - Get events with optional timestamp filter
- `subscribe_user_events(user_id)` - Real-time event stream (alternative approach)
- `get_state_events(room_id)` - Get room state events

### PresenceRepository API
Location: [`packages/surrealdb/src/repository/presence.rs`](../packages/surrealdb/src/repository/presence.rs)

Key methods:
- `get_presence_events_for_users(user_ids, since)` - Get presence for multiple users
- `get_multiple_user_presence(user_ids)` - Get current presence state
- `get_user_presence_events(user_id, since)` - Get single user's presence history

### Matrix Specification
- **Sync API**: [`./spec/client/02_rooms_users.md`](../spec/client/02_rooms_users.md)
- **Presence Format**: [`./spec/client/03_messaging_communication.md`](../spec/client/03_messaging_communication.md)

Presence fields per spec:
- `presence`: "online" | "offline" | "unavailable"
- `last_active_ago`: milliseconds since last activity
- `status_msg`: optional status message
- `currently_active`: boolean indicating active status

---

## DEFINITION OF DONE

**Task complete when**:
- ✅ EventRepository and PresenceRepository added to ClientService struct
- ✅ Events vector populated with actual room events from all joined rooms
- ✅ Presence updates populated with presence data for all users in rooms
- ✅ Empty vector TODOs removed
- ✅ Proper error handling implemented
- ✅ Code compiles successfully
- ✅ Sync response conforms to Matrix spec format

**NO REQUIREMENTS FOR**:
- ❌ Unit tests
- ❌ Integration tests
- ❌ Benchmarks
- ❌ Documentation (beyond code comments)

---

## RELATED FILES

- [`packages/client/src/repositories/client_service.rs`](../packages/client/src/repositories/client_service.rs) - Main file to modify
- [`packages/surrealdb/src/repository/event.rs`](../packages/surrealdb/src/repository/event.rs) - Event repository API
- [`packages/surrealdb/src/repository/presence.rs`](../packages/surrealdb/src/repository/presence.rs) - Presence repository API
- [`packages/surrealdb/src/repository/membership.rs`](../packages/surrealdb/src/repository/membership.rs) - Membership queries
- [`./spec/client/02_rooms_users.md`](../spec/client/02_rooms_users.md) - Matrix sync spec
- [`./spec/client/03_messaging_communication.md`](../spec/client/03_messaging_communication.md) - Presence spec
