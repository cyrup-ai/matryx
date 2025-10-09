# SPEC_CLIENT_02: Implement Read Receipts in Matrix Client Library

## Status
**NOT STARTED** - Zero implementation exists (1/10 rating)

## Objective

Implement read receipt functionality in `matryx_client` to allow applications to send read receipts to Matrix homeservers.

## Current State

- ❌ No `send_receipt()` method on MatrixClient
- ❌ No receipt types or enums defined
- ❌ No tests
- ✅ Module structure exists but is empty
- ✅ `urlencoding` dependency available
- ✅ Client compiles without errors

## Requirements

### 1. Add `send_receipt()` Method to MatrixClient

**File**: `/Volumes/samsung_t9/maxtryx/packages/client/src/lib.rs`

**Location**: After `leave_room()` method (around line 355)

**Implementation Requirements**:
- Accept parameters: `room_id: &str`, `receipt_type: &str`, `event_id: &str`, `thread_id: Option<&str>`
- Validate receipt_type is one of: `"m.read"`, `"m.read.private"`, `"m.fully_read"`
- URL encode all path parameters using `urlencoding::encode()`
- POST to: `/_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}`
- Include `thread_id` in JSON body if provided
- Use `authenticated_request()` pattern like existing methods
- Return `Result<()>`

**Minimal Working Example**:
```rust
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

### 2. Add Convenience Methods (Optional)

```rust
pub async fn send_read_receipt(&self, room_id: &str, event_id: &str) -> Result<()> {
    self.send_receipt(room_id, "m.read", event_id, None).await
}

pub async fn send_private_read_receipt(&self, room_id: &str, event_id: &str) -> Result<()> {
    self.send_receipt(room_id, "m.read.private", event_id, None).await
}

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

### 3. Add Tests

Test that:
- Client validation rejects invalid receipt types
- Method builds correct URL path with proper encoding
- Method includes thread_id in request body when provided
- Method excludes thread_id from request body when None

## Matrix Specification

**Endpoint**: `POST /_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}`

**Receipt Types**:
- `m.read` - Public read receipt (federated)
- `m.read.private` - Private read receipt (local only)
- `m.fully_read` - Fully read marker (deprecated)

**Thread ID Values**:
- `None` - Unthreaded receipt (legacy)
- `"main"` - Main timeline receipt
- `"$event_id"` - Specific thread receipt

**References**:
- Spec: https://spec.matrix.org/latest/client-server-api/#post_matrixclientv3roomsroomidreceiptreceipttypeeventid
- Local: `/Volumes/samsung_t9/maxtryx/tmp/matrix-spec/content/client-server-api/modules/receipts.md`
- Ruma Example: `/Volumes/samsung_t9/maxtryx/tmp/ruma/crates/ruma-client-api/src/receipt/create_receipt.rs`

## Definition of Done

✅ `send_receipt()` method exists on MatrixClient
✅ Validates receipt_type is valid
✅ URL encodes all path parameters
✅ Sends POST request with correct path and optional body
✅ Follows existing code patterns (authenticated_request, error handling)
✅ Code compiles without errors or warnings
✅ At least basic tests exist

## Out of Scope

- Parsing receipt events from sync (raw JSON already available in JoinedRoom.ephemeral)
- Storing receipt state
- Receipt-based notification counts
- Advanced receipt data structures
