# SPEC_CLIENT_02: Implement Read Receipts in Matrix Client Library

## Status
**INCOMPLETE** - Module structure exists but implementation is missing

## Core Objective

Implement read receipt functionality in the `matryx_client` Matrix client library to allow applications to:
1. Send read receipts to the homeserver (marking messages as read)
2. Handle read receipt events from the sync response
3. Support threaded receipts and private receipts

This is a **CLIENT library** feature - we are implementing methods for applications that connect TO Matrix homeservers, not implementing the homeserver endpoint itself.

## Current State Analysis

### What Already Exists

The module structure is scaffolded but empty:
- [`packages/client/src/_matrix/client/v3/rooms/by_room_id/receipt/mod.rs`](../../packages/client/src/_matrix/client/v3/rooms/by_room_id/receipt/mod.rs) - Empty (1 line)
- [`packages/client/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/mod.rs`](../../packages/client/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/mod.rs) - Empty (1 line)
- [`packages/client/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/by_event_id.rs`](../../packages/client/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/by_event_id.rs) - Empty (1 line)

### What's Missing

1. **No `send_receipt()` method** on `MatrixClient` struct in [`lib.rs`](../../packages/client/src/lib.rs)
2. **No receipt types** defined in the client library
3. **No receipt parsing** from sync response ephemeral events
4. **No receipt data structures** for storing/querying receipts

## Matrix Specification Reference

### Official Spec
- **Endpoint**: `POST /_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}`
- **Spec URL**: https://spec.matrix.org/latest/client-server-api/#post_matrixclientv3roomsroomidreceiptreceipttypeeventid
- **Local Spec File**: [`tmp/matrix-spec/content/client-server-api/modules/receipts.md`](../../tmp/matrix-spec/content/client-server-api/modules/receipts.md)

### Receipt Types

1. **`m.read`** - Public read receipt (federated to other users)
2. **`m.read.private`** - Private read receipt (only visible to user and their homeserver)
3. **`m.fully_read`** - Fully read marker (deprecated, use read_markers endpoint instead)

### Request Format
```json
{
  "thread_id": "main"  // Optional: "main" for main timeline, or thread root event ID
}
```

### Response Format
```json
{}  // Empty object on success
```

### Receipt Events in Sync

Receipts appear in the sync response as ephemeral events:

```json
{
  "rooms": {
    "join": {
      "!room:example.com": {
        "ephemeral": {
          "events": [
            {
              "type": "m.receipt",
              "content": {
                "$event_id": {
                  "m.read": {
                    "@user:example.com": {
                      "ts": 1661384801651,
                      "thread_id": "main"
                    }
                  }
                }
              }
            }
          ]
        }
      }
    }
  }
}
```

## Implementation Requirements

### 1. Add `send_receipt()` Method to MatrixClient

**File**: [`packages/client/src/lib.rs`](../../packages/client/src/lib.rs)

**Location**: Add after the `leave_room()` method (around line 355)

**Pattern to Follow**: Examine existing methods like `send_message()` (line 271) and `join_room()` (line 305)

**Method Signature**:
```rust
/// Send a read receipt for an event in a room
///
/// # Arguments
/// * `room_id` - The room ID
/// * `receipt_type` - Type of receipt ("m.read", "m.read.private", or "m.fully_read")
/// * `event_id` - The event ID to acknowledge up to
/// * `thread_id` - Optional thread ID ("main" for main timeline, or thread root event ID)
///
/// # Example
/// ```rust
/// client.send_receipt(
///     "!room:example.com",
///     "m.read",
///     "$event123:example.com",
///     None
/// ).await?;
/// ```
pub async fn send_receipt(
    &self,
    room_id: &str,
    receipt_type: &str,
    event_id: &str,
    thread_id: Option<&str>,
) -> Result<()> {
    // Validate receipt type
    match receipt_type {
        "m.read" | "m.read.private" | "m.fully_read" => {}
        _ => return Err(anyhow::anyhow!("Invalid receipt type: {}", receipt_type)),
    }

    let path = format!(
        "/_matrix/client/v3/rooms/{}/receipt/{}/{}",
        urlencoding::encode(room_id),
        urlencoding::encode(receipt_type),
        urlencoding::encode(event_id)
    );

    let mut body = serde_json::Map::new();
    if let Some(thread_id) = thread_id {
        body.insert("thread_id".to_string(), serde_json::Value::String(thread_id.to_string()));
    }

    let request = self.authenticated_request(reqwest::Method::POST, &path)?;
    let response = request.json(&body).send().await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow::anyhow!("Failed to send receipt: {}", error_text));
    }

    Ok(())
}
```

### 2. Add Receipt Types (Optional Enhancement)

**File**: [`packages/client/src/lib.rs`](../../packages/client/src/lib.rs)

**Location**: Add near the bottom with other type definitions (around line 460)

```rust
/// Receipt type for read receipts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptType {
    /// Public read receipt (m.read)
    Read,
    /// Private read receipt (m.read.private)
    ReadPrivate,
    /// Fully read marker (m.fully_read) - deprecated
    FullyRead,
}

impl ReceiptType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReceiptType::Read => "m.read",
            ReceiptType::ReadPrivate => "m.read.private",
            ReceiptType::FullyRead => "m.fully_read",
        }
    }
}

impl std::fmt::Display for ReceiptType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
```

Then update the `send_receipt()` signature to accept either:
```rust
pub async fn send_receipt(
    &self,
    room_id: &str,
    receipt_type: impl AsRef<str>,  // Can accept &str or ReceiptType
    event_id: &str,
    thread_id: Option<&str>,
) -> Result<()>
```

### 3. Add Convenience Methods

**File**: [`packages/client/src/lib.rs`](../../packages/client/src/lib.rs)

Add these helper methods after `send_receipt()`:

```rust
/// Send a public read receipt (m.read)
pub async fn send_read_receipt(&self, room_id: &str, event_id: &str) -> Result<()> {
    self.send_receipt(room_id, "m.read", event_id, None).await
}

/// Send a private read receipt (m.read.private)
pub async fn send_private_read_receipt(&self, room_id: &str, event_id: &str) -> Result<()> {
    self.send_receipt(room_id, "m.read.private", event_id, None).await
}

/// Send a threaded read receipt
pub async fn send_threaded_receipt(
    &self,
    room_id: &str,
    receipt_type: &str,
    event_id: &str,
    thread_id: &str,
) -> Result<()> {
    self.send_receipt(room_id, receipt_type, event_id, Some(thread_id)).await
}
```

### 4. Receipt Data Structures (Optional Enhancement)

**File**: [`packages/client/src/lib.rs`](../../packages/client/src/lib.rs)

Add these structures for parsing receipts from sync:

```rust
/// Receipt data from m.receipt events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptData {
    /// Timestamp in milliseconds since Unix epoch
    pub ts: u64,
    /// Thread ID if this is a threaded receipt
    pub thread_id: Option<String>,
}

/// Receipts for a single event
pub type EventReceipts = std::collections::HashMap<String, std::collections::HashMap<String, ReceiptData>>;
// Structure: { receipt_type: { user_id: receipt_data } }
```

### 5. Update JoinedRoom to Parse Receipts

**File**: [`packages/client/src/lib.rs`](../../packages/client/src/lib.rs)

The `JoinedRoom` struct (line 404) already has:
```rust
pub struct JoinedRoom {
    // ...
    /// Ephemeral events (typing, receipts)
    pub ephemeral: Option<serde_json::Value>,
    // ...
}
```

This already captures receipts! They're in the `ephemeral` field. Applications can parse this JSON to extract receipt events.

For better ergonomics, you could add a helper method:

```rust
impl JoinedRoom {
    /// Extract receipt events from ephemeral events
    pub fn get_receipts(&self) -> Option<Vec<serde_json::Value>> {
        let ephemeral = self.ephemeral.as_ref()?;
        let events = ephemeral.get("events")?.as_array()?;
        
        Some(
            events
                .iter()
                .filter(|e| e.get("type").and_then(|t| t.as_str()) == Some("m.receipt"))
                .cloned()
                .collect()
        )
    }
}
```

## Reference Implementations

### Ruma (Matrix Types Library)

The official Rust Matrix types library has comprehensive receipt implementations:

1. **Receipt API Endpoint**: [`tmp/ruma/crates/ruma-client-api/src/receipt/create_receipt.rs`](../../tmp/ruma/crates/ruma-client-api/src/receipt/create_receipt.rs)
   - Shows the exact API structure
   - Includes `ReceiptType` enum with `Read`, `ReadPrivate`, `FullyRead`
   - Includes `ReceiptThread` for threading support
   - Request has `room_id`, `receipt_type`, `event_id`, and optional `thread` field

2. **Receipt Event Types**: [`tmp/ruma/crates/ruma-events/src/receipt.rs`](../../tmp/ruma/crates/ruma-events/src/receipt.rs)
   - Defines `ReceiptEventContent` structure
   - Shows how receipts are structured in the sync response
   - Includes helper methods for accessing receipt data

### Matrix Specification

The full specification is available at:
- Local: [`tmp/matrix-spec/content/client-server-api/modules/receipts.md`](../../tmp/matrix-spec/content/client-server-api/modules/receipts.md)
- Online: https://spec.matrix.org/latest/client-server-api/#receipts

Key sections:
- **Threaded Read Receipts**: Explains `main` timeline vs thread roots
- **Private Read Receipts**: How `m.read.private` differs from `m.read`
- **Client Behavior**: When and how to send receipts
- **Sync Integration**: How receipts appear in ephemeral events

## Threading Support

### Thread ID Values

1. **Unthreaded receipts**: No `thread_id` field (or `None`)
   - Applies to all events regardless of threads
   - Legacy behavior from pre-threading days

2. **Main timeline receipts**: `thread_id = "main"`
   - Applies to events NOT in a thread
   - Thread roots are in the main timeline

3. **Thread receipts**: `thread_id = "$thread_root_event_id"`
   - Applies to events within a specific thread
   - Uses the thread root event ID as the identifier

### Example Usage

```rust
// Mark event as read in main timeline
client.send_receipt("!room:example.com", "m.read", "$event", Some("main")).await?;

// Mark event as read in a specific thread
client.send_receipt("!room:example.com", "m.read", "$event", Some("$thread_root")).await?;

// Mark event as read (unthreaded, legacy)
client.send_receipt("!room:example.com", "m.read", "$event", None).await?;
```

## What Changes in ./src Files

### Required Changes

**File: `packages/client/src/lib.rs`**
- Add `send_receipt()` method to `MatrixClient` impl block (after line 355)
- Add convenience methods: `send_read_receipt()`, `send_private_read_receipt()`, `send_threaded_receipt()`

### Optional Enhancements

**File: `packages/client/src/lib.rs`**
- Add `ReceiptType` enum with `Read`, `ReadPrivate`, `FullyRead` variants
- Add `ReceiptData` struct for parsing receipt timestamps and thread IDs
- Add `get_receipts()` helper method to `JoinedRoom` impl

**Files: Module structure (already exists, currently empty)**
- These files can remain empty or be used for future endpoint-specific implementations
- `packages/client/src/_matrix/client/v3/rooms/by_room_id/receipt/mod.rs`
- `packages/client/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/mod.rs`
- `packages/client/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/by_event_id.rs`

## Definition of Done

This task is complete when:

1. The `send_receipt()` method exists on `MatrixClient` and:
   - Accepts room_id, receipt_type, event_id, and optional thread_id parameters
   - Validates receipt_type is one of the supported values
   - Constructs the correct API path with URL encoding
   - Sends POST request with optional thread_id in request body
   - Returns `Ok(())` on success or appropriate error on failure

2. Applications using `matryx_client` can successfully:
   - Send public read receipts: `client.send_receipt(room, "m.read", event, None).await`
   - Send private read receipts: `client.send_receipt(room, "m.read.private", event, None).await`
   - Send threaded receipts: `client.send_receipt(room, "m.read", event, Some("main")).await`

3. The implementation follows the existing code patterns in `lib.rs`:
   - Uses `authenticated_request()` for HTTP calls
   - Follows the error handling pattern of `send_message()` and similar methods
   - Returns `Result<()>` with appropriate error messages

4. The code compiles without errors or warnings

## Out of Scope

The following are explicitly NOT required for this task:

- Parsing receipt events from sync response (the raw JSON is already available in `JoinedRoom.ephemeral`)
- Storing receipt state in a database or cache
- Notification count calculations based on receipts
- Receipt deduplication or conflict resolution
- Validation that event_id exists in the room
- Validation that user is a member of the room

These features can be added in future tasks if needed.

## Related Specifications

- **Read Markers**: [`tmp/matrix-spec/content/client-server-api/modules/read_markers.md`](../../tmp/matrix-spec/content/client-server-api/modules/read_markers.md)
  - Related but separate feature for `m.fully_read` markers
  - Uses different endpoint: `POST /_matrix/client/v3/rooms/{roomId}/read_markers`
  
- **Threading**: [`tmp/matrix-spec/content/client-server-api/modules/threading.md`](../../tmp/matrix-spec/content/client-server-api/modules/threading.md)
  - Explains thread relationships and thread IDs
  - Important for understanding threaded receipts

- **Notifications**: Receipts affect notification counts
  - The most recent receipt (public or private) determines the read-up-to point
  - Events after this point contribute to unread count
