# SPEC_CLIENT_02: Implement Read Receipts Endpoint

## Status
**MISSING** - Endpoint not implemented

## Spec Requirement
Implement `POST /_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}` endpoint for read receipts.

### Spec Reference
- **Spec Section**: Receipts (03_messaging_communication.md)
- **Endpoint**: `POST /_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}`
- **Authentication**: Required

### Receipt Types
- `m.read` - Public read receipt
- `m.read.private` - Private read receipt (not shared with other users)
- `m.fully_read` - Fully read marker (deprecated in favor of read markers endpoint)

### Request Format
```json
{
  "thread_id": "main"  // Optional: for threaded read receipts
}
```

### Response Format
```json
{}
```

## Current Implementation
**NONE** - No receipts endpoint exists in the codebase.

Search results:
- No `receipt` directory found in `/packages/server/src/_matrix/client/v3/rooms/by_room_id/`
- No receipt-related files in client implementation

## Implementation Requirements

### 1. Create Endpoint Handler
**Location**: `/packages/server/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/by_event_id.rs`

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
pub struct ReceiptRequest {
    #[serde(default)]
    pub thread_id: Option<String>, // "main" or thread event ID
}

/// POST /_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}
pub async fn post(
    State(state): State<AppState>,
    Path((room_id, receipt_type, event_id)): Path<(String, String, String)>,
    auth: AuthenticatedUser,
    Json(request): Json<ReceiptRequest>,
) -> Result<Json<Value>, StatusCode> {
    // Validate receipt type
    match receipt_type.as_str() {
        "m.read" | "m.read.private" | "m.fully_read" => {},
        _ => return Err(StatusCode::BAD_REQUEST),
    }
    
    // TODO: Verify user is member of room
    // TODO: Verify event exists in room
    // TODO: Store receipt in database
    // TODO: Broadcast receipt to room members (if not private)
    
    Ok(Json(json!({})))
}
```

### 2. Create Module Structure
**Location**: `/packages/server/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/mod.rs`

```rust
pub mod by_event_id;
```

**Location**: `/packages/server/src/_matrix/client/v3/rooms/by_room_id/receipt/mod.rs`

```rust
pub mod by_receipt_type;
```

### 3. Update Parent Module
**File**: `/packages/server/src/_matrix/client/v3/rooms/by_room_id/mod.rs`

Add:
```rust
pub mod receipt;
```

### 4. Register Route
**File**: `/packages/server/src/main.rs` or routing configuration

Add route:
```rust
.route(
    "/_matrix/client/v3/rooms/:room_id/receipt/:receipt_type/:event_id",
    post(v3::rooms::by_room_id::receipt::by_receipt_type::by_event_id::post)
)
```

### 5. Database/Repository Support
Create receipts repository:

**Location**: `/packages/surrealdb/src/repository/receipts.rs`

```rust
pub struct ReceiptsRepository {
    db: Surreal<Any>,
}

impl ReceiptsRepository {
    pub async fn set_receipt(
        &self,
        room_id: &str,
        user_id: &str,
        receipt_type: &str,
        event_id: &str,
        thread_id: Option<&str>,
        ts: u64,
    ) -> Result<(), Error> {
        // Store receipt with timestamp
        // Handle threading if thread_id is present
    }
    
    pub async fn get_receipts_for_event(
        &self,
        room_id: &str,
        event_id: &str,
    ) -> Result<HashMap<String, HashMap<String, ReceiptData>>, Error> {
        // Get all receipts for an event
        // Format: { receipt_type: { user_id: { ts, thread_id? } } }
    }
    
    pub async fn get_user_receipts(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<HashMap<String, ReceiptData>, Error> {
        // Get all receipts for a user in a room
    }
}
```

### 6. Sync Integration
**File**: `/packages/server/src/_matrix/client/v3/sync/handlers.rs`

Add receipts to sync response:
```rust
// In ephemeral events section for public receipts
let receipts = receipts_repo.get_new_receipts(&room_id, since_token).await?;
if !receipts.is_empty() {
    ephemeral_events.push(json!({
        "type": "m.receipt",
        "content": receipts
    }));
}
```

### 7. m.receipt Event Schema
Receipts appear in sync as ephemeral events:

```json
{
  "type": "m.receipt",
  "content": {
    "$event_id": {
      "m.read": {
        "@user:example.com": {
          "ts": 1661384801651
        }
      },
      "m.read.private": {
        "@user:example.com": {
          "ts": 1661384801651,
          "thread_id": "main"
        }
      }
    }
  }
}
```

## Verification Steps

1. **Endpoint exists and accepts POST**:
   ```bash
   curl -X POST 'http://localhost:8008/_matrix/client/v3/rooms/!room:example.com/receipt/m.read/$event123' \
     -H 'Authorization: Bearer <token>' \
     -H 'Content-Type: application/json' \
     -d '{}'
   ```

2. **Returns empty JSON on success**:
   ```json
   {}
   ```

3. **Public receipts appear in /sync**:
   - Check `rooms.join.<room_id>.ephemeral.events` for `m.receipt` event
   - Verify event ID and user ID in receipt data

4. **Private receipts don't broadcast**:
   - Post `m.read.private` receipt
   - Verify it doesn't appear in other users' /sync

5. **Threaded receipts work**:
   - Post receipt with `thread_id`
   - Verify thread_id is stored and returned correctly

6. **Invalid receipt types rejected**:
   - POST with invalid receipt type
   - Should return 400 Bad Request

## Related Spec Requirements
- m.receipt event schema
- Ephemeral events in /sync  
- Private read receipts (m.read.private)
- Threaded read receipts
- Read markers endpoint (separate, see SPEC_CLIENT_03)

## Dependencies
- Receipts table in database
- Sync endpoint integration
- Room membership verification

## Priority
**HIGH** - Read receipts are core messaging UX feature
