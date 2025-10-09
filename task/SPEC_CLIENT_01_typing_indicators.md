# SPEC_CLIENT_01: Complete Typing Indicators Implementation

## Status
**PARTIALLY IMPLEMENTED** - Endpoint exists but critical sync integration is missing

## Current Implementation Analysis

### What Exists ✓

1. **PUT Endpoint Handler** - [packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/by_user_id.rs](../../packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/by_user_id.rs)
   - Route registered in main.rs line 526: `.route("/v3/rooms/{room_id}/typing/{user_id}", put(...))`
   - Validates user authorization (forbids setting typing for other users)
   - Sends typing EDUs to federated servers via outbound queue
   - **CRITICAL GAP**: Does NOT store typing state locally for same-server users

2. **Federation EDU Processing** - [packages/surrealdb/src/repository/federation.rs](../../packages/surrealdb/src/repository/federation.rs#L1330-L1377)
   - `process_typing_edu()` method exists (line 1330)
   - Stores typing events in `typing_events` table with expiration
   - Called by federation handler in [packages/server/src/_matrix/federation/v1/send/by_txn_id.rs](../../packages/server/src/_matrix/federation/v1/send/by_txn_id.rs#L608)
   - Properly validates user membership before storing

3. **Database Schema**
   - `typing_events` table exists with fields:
     - `room_id`: Room identifier
     - `user_id`: User who is typing
     - `server_name`: Origin server
     - `started_at`: When typing started
     - `expires_at`: When typing status expires (30s timeout)

### What's Missing ✗

1. **Local Storage in PUT Handler**
   - PUT endpoint doesn't call `process_typing_edu` for local users
   - Only federates to remote servers, no local state tracking
   - Local clients in the same room never see typing indicators

2. **Sync Integration**
   - [packages/surrealdb/src/repository/sync.rs](../../packages/surrealdb/src/repository/sync.rs#L1102-L1132)
   - `get_room_ephemeral_events_internal()` queries `ephemeral_events` table
   - **Does NOT query** `typing_events` table
   - Typing data exists in DB but never reaches clients via /sync

3. **Typing Event Format Construction**
   - Sync must construct Matrix-spec m.typing event:
   ```json
   {
     "type": "m.typing",
     "content": {
       "user_ids": ["@alice:example.com", "@bob:example.com"]
     }
   }
   ```
   - Current code doesn't aggregate active typing users into this format

4. **Background Cleanup Task**
   - No automatic cleanup of expired typing events
   - Relies on query-time filtering (expires_at check)
   - Could accumulate stale data over time

## Matrix Spec Requirement

**Spec Section**: Typing Notifications  
**Endpoint**: `PUT /_matrix/client/v3/rooms/{roomId}/typing/{userId}`  
**Authentication**: Required

### Request Format
```json
{
  "typing": true,
  "timeout": 30000
}
```

### Response Format
```json
{}
```

### Sync Response Format (ephemeral events)
```json
{
  "rooms": {
    "join": {
      "!room:example.com": {
        "ephemeral": {
          "events": [
            {
              "type": "m.typing",
              "content": {
                "user_ids": ["@alice:example.com"]
              }
            }
          ]
        }
      }
    }
  }
}
```

## Required Implementation Changes

### 1. Update PUT Endpoint Handler

**File**: [packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/by_user_id.rs](../../packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/by_user_id.rs)

**Change**: Add local storage before federation

```rust
use matryx_surrealdb::repository::FederationRepository;

pub async fn put(
    Path((room_id, user_id)): Path<(String, String)>,
    Extension(state): Extension<AppState>,
    Extension(auth_user): Extension<AuthenticatedUser>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // ... existing authorization check ...

    let typing = payload.get("typing").and_then(|v| v.as_bool()).unwrap_or(false);
    
    // NEW: Store typing state locally BEFORE federation
    let federation_repo = FederationRepository::new(state.db.clone());
    let server_name = state.homeserver_name.as_str();
    
    if let Err(e) = federation_repo
        .process_typing_edu(&room_id, &user_id, server_name, typing)
        .await 
    {
        error!("Failed to store local typing state: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    // ... existing federation code ...
    
    Ok(Json(json!({})))
}
```

**Pattern Reference**: See how [packages/server/src/_matrix/federation/v1/send/by_txn_id.rs](../../packages/server/src/_matrix/federation/v1/send/by_txn_id.rs#L608-L615) calls `process_typing_edu` for incoming federation events.

### 2. Update Sync Ephemeral Events Query

**File**: [packages/surrealdb/src/repository/sync.rs](../../packages/surrealdb/src/repository/sync.rs#L1102-L1132)

**Method**: `get_room_ephemeral_events_internal`

**Change**: Query typing_events and construct m.typing event format

```rust
async fn get_room_ephemeral_events_internal(
    &self,
    room_id: &str,
    since: Option<&SyncPosition>,
) -> Result<Vec<Value>, RepositoryError> {
    let mut ephemeral_events = Vec::new();
    
    // Query active typing users (not expired)
    let typing_query = r#"
        SELECT user_id FROM typing_events
        WHERE room_id = $room_id 
        AND expires_at > time::now()
        ORDER BY started_at DESC
    "#;
    
    let mut typing_response = self.db
        .query(typing_query)
        .bind(("room_id", room_id.to_string()))
        .await
        .map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_typing_users".to_string(),
        })?;
    
    #[derive(serde::Deserialize)]
    struct TypingUser {
        user_id: String,
    }
    
    let typing_users: Vec<TypingUser> = typing_response
        .take(0)
        .map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "parse_typing_users".to_string(),
        })?;
    
    // Construct m.typing event if there are active typers
    if !typing_users.is_empty() {
        let user_ids: Vec<String> = typing_users
            .into_iter()
            .map(|u| u.user_id)
            .collect();
        
        ephemeral_events.push(json!({
            "type": "m.typing",
            "content": {
                "user_ids": user_ids
            }
        }));
    }
    
    // Query other ephemeral events (receipts, etc.) from existing table
    let other_query = if since.is_some() {
        r#"
            SELECT * FROM ephemeral_events 
            WHERE room_id = $room_id AND timestamp > $since_timestamp
            ORDER BY timestamp DESC 
            LIMIT 10
        "#
    } else {
        r#"
            SELECT * FROM ephemeral_events 
            WHERE room_id = $room_id 
            ORDER BY timestamp DESC 
            LIMIT 10
        "#
    };
    
    let mut query_builder = self.db
        .query(other_query)
        .bind(("room_id", room_id.to_string()));
    
    if let Some(sync_pos) = since {
        query_builder = query_builder.bind(("since_timestamp", sync_pos.timestamp));
    }
    
    let mut response = query_builder.await
        .map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_other_ephemeral_events".to_string(),
        })?;
    
    let other_events: Vec<Value> = response
        .take(0)
        .map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "parse_other_ephemeral_events".to_string(),
        })?;
    
    ephemeral_events.extend(other_events);
    
    Ok(ephemeral_events)
}
```

**Pattern Reference**: See how [packages/surrealdb/src/repository/federation.rs](../../packages/surrealdb/src/repository/federation.rs#L1330-L1377) queries and manages typing_events.

### 3. Optional: Background Cleanup Task

**File**: Create new file `packages/server/src/tasks/typing_cleanup.rs`

**Purpose**: Periodically delete expired typing events to prevent table bloat

```rust
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error};
use crate::AppState;

pub async fn start_typing_cleanup_task(state: AppState) {
    let mut interval = interval(Duration::from_secs(60)); // Run every 60 seconds
    
    loop {
        interval.tick().await;
        
        let query = "DELETE typing_events WHERE expires_at < time::now()";
        
        match state.db.query(query).await {
            Ok(mut result) => {
                if let Ok(count) = result.take::<Option<i64>>(0) {
                    debug!("Cleaned up {} expired typing events", count.unwrap_or(0));
                }
            }
            Err(e) => {
                error!("Failed to cleanup typing events: {}", e);
            }
        }
    }
}
```

**Integration**: Spawn task in `packages/server/src/main.rs`:

```rust
// In main() function after database connection
tokio::spawn(tasks::typing_cleanup::start_typing_cleanup_task(app_state.clone()));
```

**Pattern Reference**: See how the outbound federation queue is spawned as a background task in main.rs.

## Data Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ LOCAL USER TYPES                                            │
└─────────────────────────────────────────────────────────────┘
                    │
                    ▼
     PUT /v3/rooms/{room}/typing/{user}
                    │
         ┌──────────┴──────────┐
         │                     │
         ▼                     ▼
   Store locally         Send EDU to
   (typing_events)      remote servers
         │                     │
         │                     │
         ▼                     ▼
    ┌────────────┐      ┌──────────┐
    │ Local DB   │      │ Remote   │
    │ Storage    │      │ Servers  │
    └────────────┘      └──────────┘
         │                     │
         │                     │
         │              (Remote stores
         │               in their DB)
         │                     │
         └──────────┬──────────┘
                    │
                    ▼
         Client calls /sync
                    │
                    ▼
    Query typing_events WHERE
    expires_at > now()
                    │
                    ▼
    Construct m.typing event
    with user_ids array
                    │
                    ▼
    Return in ephemeral events
                    │
                    ▼
         Client displays typing
```

## Definition of Done

### Functional Requirements

1. **Local Typing Indicators Work**
   - User A types in room → PUT endpoint succeeds
   - User B in same room calls /sync → sees User A in m.typing event
   - User A stops typing → User B's next /sync shows empty user_ids array

2. **Federated Typing Works**
   - User on remote server types → local server receives EDU
   - Local users see remote user in m.typing via /sync
   - Timeout respected (remote user removed after 30s)

3. **Authorization Enforced**
   - User cannot set typing status for other users (403 Forbidden)
   - Only authenticated users can call endpoint (401 Unauthorized)

4. **Timeout Behavior**
   - Typing status expires after timeout duration (default 30s)
   - Expired users not included in /sync m.typing events
   - Setting typing=false immediately removes user from typing list

5. **Multiple Users**
   - Multiple users can be typing simultaneously
   - m.typing event contains all currently typing users
   - Order doesn't matter but should be consistent

### Code Changes Summary

**Must Change**:
- `packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/by_user_id.rs` - Add local storage call
- `packages/surrealdb/src/repository/sync.rs` - Query typing_events and construct m.typing format

**Should Change**:
- `packages/server/src/main.rs` - Add typing cleanup background task
- Create `packages/server/src/tasks/typing_cleanup.rs` - Cleanup task implementation

**Already Correct** (no changes needed):
- Route registration in main.rs
- Federation EDU processing in federation.rs
- Database schema for typing_events

### Verification Steps

1. **Start typing indicator**:
   ```bash
   curl -X PUT 'http://localhost:8008/_matrix/client/v3/rooms/!room:example.com/typing/@alice:example.com' \
     -H 'Authorization: Bearer <alice_token>' \
     -H 'Content-Type: application/json' \
     -d '{"typing": true, "timeout": 30000}'
   ```
   Expected: Returns `{}`

2. **Check /sync shows typing**:
   ```bash
   curl -X GET 'http://localhost:8008/_matrix/client/v3/sync' \
     -H 'Authorization: Bearer <bob_token>'
   ```
   Expected: Response includes:
   ```json
   {
     "rooms": {
       "join": {
         "!room:example.com": {
           "ephemeral": {
             "events": [{
               "type": "m.typing",
               "content": {"user_ids": ["@alice:example.com"]}
             }]
           }
         }
       }
     }
   }
   ```

3. **Stop typing**:
   ```bash
   curl -X PUT 'http://localhost:8008/_matrix/client/v3/rooms/!room:example.com/typing/@alice:example.com' \
     -H 'Authorization: Bearer <alice_token>' \
     -H 'Content-Type: application/json' \
     -d '{"typing": false}'
   ```
   Expected: Next /sync shows `user_ids: []`

4. **Authorization check**:
   ```bash
   curl -X PUT 'http://localhost:8008/_matrix/client/v3/rooms/!room:example.com/typing/@bob:example.com' \
     -H 'Authorization: Bearer <alice_token>' \
     -H 'Content-Type: application/json' \
     -d '{"typing": true}'
   ```
   Expected: 403 Forbidden

## Related Files Reference

### Existing Implementation
- [packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/by_user_id.rs](../../packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/by_user_id.rs) - PUT endpoint handler
- [packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/mod.rs](../../packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/mod.rs) - Module declaration
- [packages/surrealdb/src/repository/federation.rs](../../packages/surrealdb/src/repository/federation.rs#L1330-L1377) - `process_typing_edu` method
- [packages/server/src/_matrix/federation/v1/send/by_txn_id.rs](../../packages/server/src/_matrix/federation/v1/send/by_txn_id.rs#L608) - Federation EDU handler

### Needs Modification
- [packages/surrealdb/src/repository/sync.rs](../../packages/surrealdb/src/repository/sync.rs#L1102) - Ephemeral events query
- [packages/server/src/_matrix/client/v3/sync/data.rs](../../packages/server/src/_matrix/client/v3/sync/data.rs#L149) - Calls sync repository
- [packages/server/src/_matrix/client/v3/sync/handlers.rs](../../packages/server/src/_matrix/client/v3/sync/handlers.rs#L344) - Main sync handler

### Pattern Examples
- See [packages/surrealdb/src/repository/federation.rs](../../packages/surrealdb/src/repository/federation.rs#L1382) for receipt EDU processing (similar pattern)
- See [packages/server/src/main.rs](../../packages/server/src/main.rs) for background task spawning examples
- See [packages/server/src/_matrix/client/v3/sync/handlers.rs](../../packages/server/src/_matrix/client/v3/sync/handlers.rs) for sync response construction

## Implementation Notes

### Why Typing Events Use Separate Table

The `typing_events` table is separate from `ephemeral_events` because:
1. Typing has unique expiration semantics (automatic 30s timeout)
2. Needs efficient querying by room and expiration time
3. Matrix spec requires aggregation (all users typing → single event)
4. Different from other ephemeral events like receipts

### Expiration Strategy

Two-phase approach:
1. **Query-time filtering**: `WHERE expires_at > time::now()` ensures only active typers returned
2. **Background cleanup**: Optional periodic DELETE to prevent table growth

Query-time filtering is sufficient for correctness. Background cleanup is optimization.

### Federation Considerations

The existing code already handles federation correctly:
- Outgoing: PUT endpoint sends EDUs to remote servers
- Incoming: Federation handler processes EDUs from remote servers
- Local implementation just fills the local gap

## Priority
**HIGH** - Critical for user experience in active conversations. Endpoint exists but incomplete implementation means feature appears broken to users.
