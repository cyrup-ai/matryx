# RECEIPT_1: Implement Read Receipts Processing

## OBJECTIVE

Remove the "for now" stub at `packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs:71` by implementing proper read receipts processing that complies with the Matrix 1.4 specification.

## CONTEXT

**File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs`  
**Line:** 71  
**Current Code:**
```rust
// Note: m.read and m.read.private are read receipts, which should be handled
// by the receipts endpoint. For now, we acknowledge but don't process them.
if payload.read.is_some() || payload.read_private.is_some() {
    info!("Read receipts provided but will be handled by receipts endpoint");
}
```

Currently read receipts are acknowledged but not processed. **This violates the Matrix 1.4 specification which explicitly requires that the `/read_markers` endpoint MUST process `m.read` and `m.read.private` the same way as the receipts endpoint.**

## MATRIX SPECIFICATION REQUIREMENTS

From Matrix 1.4 Client-Server API specification ([../../tmp/matrix-spec-official/content/client-server-api/modules/read_markers.md](../../tmp/matrix-spec-official/content/client-server-api/modules/read_markers.md)):

> **Server behaviour**: The server must additionally ensure that it treats the presence of `m.read` and `m.read.private` in the `/read_markers` request the same as how it would for a request to `/receipt/{receiptType}/{eventId}`.

This is not optional - it's a MUST requirement.

## ARCHITECTURE RESEARCH

### Receipt Repository Infrastructure

The codebase already has complete receipts infrastructure:

**Receipt Repository:** [../../packages/surrealdb/src/repository/receipt.rs](../../packages/surrealdb/src/repository/receipt.rs)

```rust
pub struct ReceiptRepository {
    db: Surreal<Any>,
}

impl ReceiptRepository {
    pub async fn store_receipt(
        &self,
        room_id: &str,
        user_id: &str,
        event_id: &str,
        receipt_type: &str,      // "m.read" or "m.read.private"
        thread_id: Option<&str>,  // Threading support (Matrix 1.4)
        server_name: &str,
    ) -> Result<(), RepositoryError>
}
```

**Database Schema:** [../../packages/surrealdb/migrations/tables/089_receipts.surql](../../packages/surrealdb/migrations/tables/089_receipts.surql)

The receipts table stores:
- `room_id`, `user_id`, `event_id`
- `receipt_type` ("m.read" or "m.read.private")
- `thread_id` (Optional, for threaded receipts)
- `timestamp` (auto-generated)
- `is_private` (boolean flag)
- `server_name` (homeserver identifier)
- `received_at` (DateTime)

**Entity Types:**
- [../../packages/entity/src/types/read_receipt_metadata.rs](../../packages/entity/src/types/read_receipt_metadata.rs)
- [../../packages/entity/src/types/user_read_receipt.rs](../../packages/entity/src/types/user_read_receipt.rs)
- [../../packages/entity/src/types/room_receipts.rs](../../packages/entity/src/types/room_receipts.rs)
- [../../packages/entity/src/types/receipt_edu.rs](../../packages/entity/src/types/receipt_edu.rs)

### Reference Implementation

The receipts endpoint shows the exact pattern to follow:

**File:** [../../packages/server/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/by_event_id.rs](../../packages/server/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/by_event_id.rs)

Key implementation details:

```rust
// 1. Instantiate repository
let receipt_repo = ReceiptRepository::new(state.db.clone());

// 2. Store public read receipt
match receipt_type.as_str() {
    "m.read" => {
        receipt_repo
            .store_receipt(
                &room_id,
                &user.user_id,
                &event_id,
                "m.read",
                thread_id.as_deref(),
                &state.homeserver_name,
            )
            .await?;
        
        // Note: receipts endpoint also handles federation
        // read_markers does NOT need to federate (receipts endpoint handles that)
    },
    
    "m.read.private" => {
        receipt_repo
            .store_receipt(
                &room_id,
                &user.user_id,
                &event_id,
                "m.read.private",
                thread_id.as_deref(),
                &state.homeserver_name,
            )
            .await?;
        
        // Private receipts are NEVER federated per Matrix spec
    },
}
```

### Import Requirements

From the receipts endpoint, you need:

```rust
use matryx_surrealdb::repository::ReceiptRepository;
```

The repository is already exported in [../../packages/surrealdb/src/repository/mod.rs](../../packages/surrealdb/src/repository/mod.rs):

```rust
pub use receipt::*;  // Line 168
```

## IMPLEMENTATION SPECIFICATION

### Changes Required in read_markers.rs

**Location:** Line 71 in [../../packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs](../../packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs)

**What to Change:**

Replace the stub (lines 70-73):
```rust
// Note: m.read and m.read.private are read receipts, which should be handled
// by the receipts endpoint. For now, we acknowledge but don't process them.
if payload.read.is_some() || payload.read_private.is_some() {
    info!("Read receipts provided but will be handled by receipts endpoint");
}
```

With actual implementation:

```rust
// Process read receipts as required by Matrix 1.4 specification
// The server MUST treat m.read and m.read.private in /read_markers the same
// as it would for requests to /receipt/{receiptType}/{eventId}
let receipt_repo = ReceiptRepository::new(state.db.clone());

// Process m.read (public read receipt)
if let Some(ref event_id) = payload.read {
    match receipt_repo
        .store_receipt(
            &room_id,
            &user_id,
            event_id,
            "m.read",
            None,  // read_markers payload doesn't include thread_id
            &state.homeserver_name,
        )
        .await
    {
        Ok(()) => {
            debug!("Stored m.read receipt for user {} in room {} at event {}", user_id, room_id, event_id);
        },
        Err(e) => {
            error!("Failed to store m.read receipt: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
}

// Process m.read.private (private read receipt)
if let Some(ref event_id) = payload.read_private {
    match receipt_repo
        .store_receipt(
            &room_id,
            &user_id,
            event_id,
            "m.read.private",
            None,  // read_markers payload doesn't include thread_id
            &state.homeserver_name,
        )
        .await
    {
        Ok(()) => {
            debug!("Stored m.read.private receipt for user {} in room {} at event {}", user_id, room_id, event_id);
        },
        Err(e) => {
            error!("Failed to store m.read.private receipt: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
}
```

### Required Imports to Add

At the top of [../../packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs](../../packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs), add:

```rust
use matryx_surrealdb::repository::ReceiptRepository;
```

Change the logging import from:
```rust
use tracing::{error, info, warn};
```

To:
```rust
use tracing::{debug, error, warn};
```

### Key Implementation Notes

1. **Thread ID Support:** The current `ReadMarkersRequest` struct doesn't include `thread_id`, so pass `None`. This is correct for the read_markers endpoint (threading is supported in the receipts endpoint).

2. **Federation:** The read_markers endpoint does NOT need to implement federation. The receipts endpoint (`/receipt/m.read/{eventId}`) handles federation separately. The Matrix spec requires read_markers to store receipts the same way, but NOT necessarily federate them.

3. **Error Handling:** Use proper error handling with `match` and return `StatusCode::INTERNAL_SERVER_ERROR` on database errors. Never use `unwrap()` or `expect()`.

4. **Logging:** Use `debug!` level for successful operations (not `info!`), and `error!` for failures.

5. **Receipt Type Strings:** Use exactly `"m.read"` and `"m.read.private"` as the receipt type strings.

6. **Server Name:** Pass `&state.homeserver_name` which is available in the function scope.

## IMPLEMENTATION STEPS

### Step 1: Add Import

Add to imports section (approximately line 5):
```rust
use matryx_surrealdb::repository::ReceiptRepository;
```

Update tracing import to include `debug`:
```rust
use tracing::{debug, error, warn};
```

### Step 2: Replace Stub Code

Locate lines 70-73 and replace with the implementation code shown above.

### Step 3: Verify Error Handling

Ensure:
- No `unwrap()` or `expect()` calls
- Proper `match` expressions for error handling
- Return `StatusCode::INTERNAL_SERVER_ERROR` on database errors
- Use `error!` macro for logging failures
- Use `debug!` macro for logging successes

### Step 4: Verify Compilation

After changes:
```bash
cd /Volumes/samsung_t9/maxtryx
cargo check -p matryx_server
```

Expected result: File compiles without errors.

## REFERENCE IMPLEMENTATIONS

### Synapse (Python Reference)

From [../../tmp/synapse/synapse/handlers/receipts.py](../../tmp/synapse/synapse/handlers/receipts.py):

```python
async def received_client_receipt(
    self,
    room_id: str,
    receipt_type: str,
    user_id: UserID,
    event_id: str,
    thread_id: Optional[str],  # Optional threading support
) -> None:
    receipt = ReadReceipt(
        room_id=room_id,
        receipt_type=receipt_type,
        user_id=user_id.to_string(),
        event_ids=[event_id],
        thread_id=thread_id,
        data={"ts": int(self.clock.time_msec())},
    )

    is_new = await self._handle_new_receipts([receipt])
    if not is_new:
        return

    # Only federate public receipts (not READ_PRIVATE)
    if self.federation_sender and receipt_type != ReceiptTypes.READ_PRIVATE:
        await self.federation_sender.send_read_receipt(receipt)
```

### Matrix Rust SDK

From [../../tmp/matrix-rust-sdk/crates/matrix-sdk-base/src/read_receipts.rs](../../tmp/matrix-rust-sdk/crates/matrix-sdk-base/src/read_receipts.rs) for reference on data structures.

## DEFINITION OF DONE

- [ ] Stub comment "For now, we acknowledge but don't process them" is removed
- [ ] `ReceiptRepository` import is added
- [ ] `debug` is added to tracing imports
- [ ] `m.read` receipts are stored when `payload.read` is present
- [ ] `m.read.private` receipts are stored when `payload.read_private` is present
- [ ] Both receipt types call `store_receipt()` with correct parameters
- [ ] Error handling uses `match` expressions (no `unwrap()`/`expect()`)
- [ ] Database errors return `StatusCode::INTERNAL_SERVER_ERROR`
- [ ] Success logging uses `debug!` level (not `info!`)
- [ ] Error logging uses `error!` level
- [ ] `thread_id` parameter is `None` (correct for read_markers)
- [ ] Code compiles without errors (`cargo check -p matryx_server`)
- [ ] Matrix 1.4 specification requirement is satisfied

## CONSTRAINTS

- **DO NOT** write any test code
- **DO NOT** write any benchmark code  
- **DO NOT** modify test files
- **DO NOT** add documentation files
- **ONLY** modify production source code in `packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs`
- **DO NOT** use `unwrap()` or `expect()`
- **DO NOT** implement federation in read_markers (receipts endpoint handles that)
- **DO NOT** add thread_id support to ReadMarkersRequest (not required by spec)

## CITATIONS

- Matrix 1.4 Spec (Read Markers): [../../tmp/matrix-spec-official/content/client-server-api/modules/read_markers.md](../../tmp/matrix-spec-official/content/client-server-api/modules/read_markers.md)
- Matrix 1.4 Spec (Receipts API): [../../tmp/matrix-spec-official/data/api/client-server/receipts.yaml](../../tmp/matrix-spec-official/data/api/client-server/receipts.yaml)
- Receipt Repository: [../../packages/surrealdb/src/repository/receipt.rs](../../packages/surrealdb/src/repository/receipt.rs)
- Receipts Endpoint (Reference): [../../packages/server/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/by_event_id.rs](../../packages/server/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/by_event_id.rs)
- Current File: [../../packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs](../../packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs)
- Database Migration: [../../packages/surrealdb/migrations/tables/089_receipts.surql](../../packages/surrealdb/migrations/tables/089_receipts.surql)
- Synapse Reference: [../../tmp/synapse/synapse/handlers/receipts.py](../../tmp/synapse/synapse/handlers/receipts.py)
- Repository Module Exports: [../../packages/surrealdb/src/repository/mod.rs](../../packages/surrealdb/src/repository/mod.rs)
