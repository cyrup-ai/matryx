# SPEC_CLIENT_01: Implement Typing Indicators Endpoint

## Status
**MISSING** - Endpoint not implemented

## Spec Requirement
Implement `PUT /_matrix/client/v3/rooms/{roomId}/typing/{userId}` endpoint for typing notifications.

### Spec Reference
- **Spec Section**: Typing Notifications (03_messaging_communication.md)
- **Endpoint**: `PUT /_matrix/client/v3/rooms/{roomId}/typing/{userId}`
- **Authentication**: Required

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

## Current Implementation
**NONE** - No typing indicators endpoint exists in the codebase.

Search results:
- No `typing` directory found in `/packages/server/src/_matrix/client/v3/rooms/by_room_id/`
- No typing-related files in client implementation

## Implementation Requirements

### 1. Create Endpoint Handler
**Location**: `/packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/by_user_id.rs`

```rust
use crate::auth::AuthenticatedUser;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
pub struct TypingRequest {
    pub typing: bool,
    #[serde(default)]
    pub timeout: Option<u64>, // milliseconds
}

/// PUT /_matrix/client/v3/rooms/{roomId}/typing/{userId}
pub async fn put(
    State(state): State<AppState>,
    Path((room_id, user_id)): Path<(String, String)>,
    auth: AuthenticatedUser,
    Json(request): Json<TypingRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Verify user is authorized
    if auth.user_id != user_id {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // TODO: Store typing state in ephemeral events
    // TODO: Broadcast typing notification to room members
    // TODO: Implement timeout mechanism
    
    Ok(Json(json!({})))
}
```

### 2. Create Module Structure
**Location**: `/packages/server/src/_matrix/client/v3/rooms/by_room_id/typing/mod.rs`

```rust
pub mod by_user_id;
```

### 3. Update Parent Module
**File**: `/packages/server/src/_matrix/client/v3/rooms/by_room_id/mod.rs`

Add:
```rust
pub mod typing;
```

### 4. Register Route
**File**: `/packages/server/src/main.rs` or routing configuration

Add route:
```rust
.route(
    "/_matrix/client/v3/rooms/:room_id/typing/:user_id",
    put(v3::rooms::by_room_id::typing::by_user_id::put)
)
```

### 5. Database/Repository Support
Create typing notifications repository:

**Location**: `/packages/surrealdb/src/repository/typing.rs`

```rust
pub struct TypingRepository {
    db: Surreal<Any>,
}

impl TypingRepository {
    pub async fn set_typing_status(
        &self,
        room_id: &str,
        user_id: &str,
        typing: bool,
        timeout_ms: Option<u64>
    ) -> Result<(), Error> {
        // Store typing state in ephemeral events table
        // Set expiry based on timeout
    }
    
    pub async fn get_typing_users(
        &self,
        room_id: &str
    ) -> Result<Vec<String>, Error> {
        // Get all users currently typing in room
    }
}
```

### 6. Sync Integration
**File**: `/packages/server/src/_matrix/client/v3/sync/handlers.rs`

Add typing events to sync response:
```rust
// In ephemeral events section
let typing_users = typing_repo.get_typing_users(&room_id).await?;
if !typing_users.is_empty() {
    ephemeral_events.push(json!({
        "type": "m.typing",
        "content": {
            "user_ids": typing_users
        }
    }));
}
```

## Verification Steps

1. **Endpoint exists**:
   ```bash
   curl -X PUT 'http://localhost:8008/_matrix/client/v3/rooms/!room:example.com/typing/@user:example.com' \
     -H 'Authorization: Bearer <token>' \
     -H 'Content-Type: application/json' \
     -d '{"typing": true, "timeout": 30000}'
   ```

2. **Returns empty JSON on success**:
   ```json
   {}
   ```

3. **Typing state appears in /sync**:
   - Check `rooms.join.<room_id>.ephemeral.events` contains `m.typing` event
   - Verify `user_ids` array contains the typing user

4. **Timeout works**:
   - Set typing status with timeout
   - Wait for timeout to expire
   - Verify user no longer appears in typing users

5. **Authorization check**:
   - Attempt to set typing for different user
   - Should return 403 Forbidden

## Related Spec Requirements
- m.typing event schema
- Ephemeral events in /sync
- Timeout mechanism (automatic cleanup)

## Dependencies
- Ephemeral events table in database
- Sync endpoint integration
- Background task for timeout cleanup

## Priority
**MEDIUM** - Nice to have for user experience, not critical for basic functionality
