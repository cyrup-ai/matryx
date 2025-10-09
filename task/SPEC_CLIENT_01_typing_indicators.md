# SPEC_CLIENT_01: Typing Indicators Implementation - CRITICAL ISSUES

## QA Rating: 2/10 - FUNDAMENTALLY BROKEN

### Critical Assessment
The typing indicators feature is NOT "partially implemented" - it is **FUNDAMENTALLY BROKEN** at the database schema level. The implementation will fail immediately in production due to table name and schema mismatches.

---

## CRITICAL ISSUE #1: Database Table Name Mismatch

**Severity**: BLOCKER - Feature completely non-functional

### Current State
- **Database schema defines**: `typing_notification` table
  - Location: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/migrations/tables/076_typing_notification.surql`
  - Fields: `room_id`, `typing`, `user_id`, `timestamp`

- **Code attempts to use**: `typing_events` table (DOES NOT EXIST)
  - Location: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/federation.rs` line 1343-1356
  - Expected fields: `room_id`, `user_id`, `server_name`, `started_at`, `expires_at`

### Impact
- ✗ Federation inbound FAILS: `process_typing_edu()` writes to non-existent table
- ✗ All typing data storage operations fail with database errors
- ✗ Feature appears broken to all users

### Required Fix
**DECISION NEEDED**: Choose ONE approach:

#### Option A: Use existing `typing_notification` table (RECOMMENDED)
1. Update `federation.rs` process_typing_edu to write to `typing_notification` table
2. Modify schema to add `expires_at` field for timeout tracking
3. Update queries to match new field names

#### Option B: Create new `typing_events` table
1. Create migration file `157_typing_events.surql` with correct schema
2. Keep federation.rs code as-is
3. More complex but separates concerns

**MUST RESOLVE THIS FIRST** before any other work can proceed.

---

## CRITICAL ISSUE #2: PUT Endpoint Missing Local Storage

**File**: `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/by_user_id.rs`

### Current Behavior
```rust
pub async fn put(...) -> Result<Json<Value>, StatusCode> {
    // Verify user authorization ✓
    // Get remote servers ✓
    // Send EDU to remote servers ✓
    // ✗ MISSING: Store typing state locally
    Ok(Json(json!({})))
}
```

### What's Missing
- No call to store typing state in database
- Local users in same room never see typing indicators
- Only federates to remote servers

### Required Implementation
Add before federation code:
```rust
use matryx_surrealdb::repository::FederationRepository;

// Store typing state locally
let federation_repo = FederationRepository::new(state.db.clone());
let server_name = state.homeserver_name.as_str();

if let Err(e) = federation_repo
    .process_typing_edu(&room_id, &user_id, server_name, typing)
    .await 
{
    error!("Failed to store local typing state: {}", e);
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
}
```

---

## CRITICAL ISSUE #3: Sync Integration Completely Missing

**File**: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/sync.rs` line 1102-1132

### Current Behavior
- Queries `event` table for ephemeral events with type filter
- Does NOT query typing data table (neither `typing_notification` nor `typing_events`)
- Returns empty typing indicators to all clients

### Required Implementation
Replace `get_room_ephemeral_events()` method to:
1. Query active typing users from typing table
2. Construct Matrix-spec `m.typing` event format:
   ```json
   {
     "type": "m.typing",
     "content": {
       "user_ids": ["@alice:example.com", "@bob:example.com"]
     }
   }
   ```
3. Combine with other ephemeral events (receipts, etc.)

### Example Query Pattern
```sql
SELECT user_id FROM typing_notification
WHERE room_id = $room_id 
AND timestamp > (time::now() - 30s)  -- Active typers only
ORDER BY timestamp DESC
```

---

## CRITICAL ISSUE #4: Schema Design Inadequate

**File**: `/Volumes/samsung_t9/maxtryx/packages/surrealdb/migrations/tables/076_typing_notification.surql`

### Current Schema Problems
```sql
DEFINE FIELD timestamp ON TABLE typing_notification TYPE datetime DEFAULT time::now();
```

**Issues**:
- No `expires_at` field for automatic timeout handling
- No `server_name` field to track federation origin
- No `started_at` field to differentiate start time from last update
- Relies on query-time filtering (`timestamp > now() - 30s`) which is fragile

### Required Schema Updates
```sql
DEFINE FIELD room_id ON TABLE typing_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE typing_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD typing ON TABLE typing_notification TYPE bool DEFAULT false;
DEFINE FIELD server_name ON TABLE typing_notification TYPE string;
DEFINE FIELD started_at ON TABLE typing_notification TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE typing_notification TYPE datetime;
DEFINE FIELD updated_at ON TABLE typing_notification TYPE datetime DEFAULT time::now();

-- Index for efficient cleanup queries
DEFINE INDEX idx_typing_expires ON TABLE typing_notification FIELDS expires_at;
DEFINE INDEX idx_typing_room ON TABLE typing_notification FIELDS room_id, expires_at;
```

---

## MISSING FEATURE: Background Cleanup Task

**Severity**: Medium - Causes table bloat over time

### Current State
- No automatic cleanup of expired typing events
- Table will accumulate stale data indefinitely
- Query performance will degrade over time

### Required Implementation
Create `/Volumes/samsung_t9/maxtryx/packages/server/src/tasks/typing_cleanup.rs`:

```rust
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error};
use crate::AppState;

pub async fn start_typing_cleanup_task(state: AppState) {
    let mut interval = interval(Duration::from_secs(60));
    
    loop {
        interval.tick().await;
        
        let query = "DELETE typing_notification WHERE expires_at < time::now()";
        
        match state.db.query(query).await {
            Ok(_) => debug!("Cleaned up expired typing events"),
            Err(e) => error!("Failed to cleanup typing events: {}", e),
        }
    }
}
```

Spawn in `main.rs`:
```rust
tokio::spawn(tasks::typing_cleanup::start_typing_cleanup_task(app_state.clone()));
```

---

## UNUSED CODE: Entity Type Not Integrated

**File**: `/Volumes/samsung_t9/maxtryx/packages/entity/src/types/typing_notification.rs`

### Current State
- `TypingNotification` struct exists
- Fields: `room_id`, `typing`, `user_id`
- **Completely unused** in any repository or handler code

### Required Integration
Update code to use entity type instead of raw queries:
```rust
use matryx_entity::types::TypingNotification;

// In repository methods
let notification = TypingNotification::new(
    room_id.to_string(),
    typing,
    user_id.to_string(),
);
```

---

## Implementation Priority Order

### Phase 1: Fix Critical Database Issues (REQUIRED FOR ANY FUNCTIONALITY)
1. **DECIDE**: Table name strategy (typing_notification vs typing_events)
2. **UPDATE**: Schema to add required fields (expires_at, server_name, started_at)
3. **MIGRATE**: Database with new schema
4. **UPDATE**: All code references to match chosen table name

### Phase 2: Complete Core Integration
5. **FIX**: PUT endpoint to store locally (federation.rs call)
6. **FIX**: Sync repository to query typing data
7. **IMPLEMENT**: M.typing event format construction

### Phase 3: Polish & Optimization
8. **CREATE**: Background cleanup task
9. **INTEGRATE**: Entity type usage
10. **TEST**: End-to-end functionality

---

## Testing Verification (AFTER FIXES)

### Test 1: Local Typing
```bash
# Alice starts typing
curl -X PUT 'http://localhost:8008/_matrix/client/v3/rooms/!room:example.com/typing/@alice:example.com' \
  -H 'Authorization: Bearer <alice_token>' \
  -d '{"typing": true, "timeout": 30000}'

# Bob syncs and should see Alice typing
curl -X GET 'http://localhost:8008/_matrix/client/v3/sync' \
  -H 'Authorization: Bearer <bob_token>'

# Expected: m.typing event with user_ids: ["@alice:example.com"]
```

### Test 2: Federation Inbound
- Verify remote typing EDUs are processed without database errors
- Check logs for successful storage
- Verify data appears in sync for local users

### Test 3: Timeout Behavior
- Set typing=true, wait 30s
- Verify user removed from typing list
- Set typing=false explicitly
- Verify immediate removal

---

## Files Requiring Changes

### CRITICAL (Must Fix)
1. `/Volumes/samsung_t9/maxtryx/packages/surrealdb/migrations/tables/076_typing_notification.surql` - Schema update
2. `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/federation.rs` - Table name & field updates
3. `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/by_user_id.rs` - Add local storage
4. `/Volumes/samsung_t9/maxtryx/packages/surrealdb/src/repository/sync.rs` - Add typing query & m.typing construction

### IMPORTANT (Should Fix)
5. `/Volumes/samsung_t9/maxtryx/packages/server/src/main.rs` - Spawn cleanup task
6. `/Volumes/samsung_t9/maxtryx/packages/server/src/tasks/typing_cleanup.rs` - Create new file

---

## Why Rating is 2/10

### What Works (2 points)
- ✓ PUT endpoint exists and handles requests correctly
- ✓ Authorization validation works
- ✓ Federation outbound (sending to remote servers) functions

### What's Broken (8 points deducted)
- ✗ **CRITICAL**: Table name mismatch breaks all database operations
- ✗ **CRITICAL**: Schema incompatibility prevents proper data storage
- ✗ **CRITICAL**: Federation inbound fails (database errors)
- ✗ **CRITICAL**: Local storage completely missing
- ✗ **CRITICAL**: Sync integration non-existent
- ✗ No background cleanup (table bloat over time)
- ✗ Entity type defined but unused
- ✗ No proper timeout expiration handling

### Production Impact
If deployed as-is:
- Local typing indicators: BROKEN (nothing stored)
- Remote typing indicators: BROKEN (database errors on receive)
- Client sync: BROKEN (no typing data returned)
- Federation: PARTIALLY WORKING (can send but can't receive)

**Conclusion**: Feature is unusable in current state. Database schema must be fixed before ANY other work can proceed.

---

/Volumes/samsung_t9/maxtryx/task/SPEC_CLIENT_01_typing_indicators.md
