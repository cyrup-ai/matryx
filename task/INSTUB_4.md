# INSTUB_4: Read Receipts Storage Implementation

**Priority**: HIGH  
**Estimated Effort**: 1 session  
**Category**: Client-Server API Completion

---

## OBJECTIVE

Implement read receipt storage (`m.read` and `m.read.private`) in the read markers endpoint by calling existing `ReceiptRepository` methods instead of ignoring the receipts.

**WHY**: Read receipts are currently acknowledged but not stored or processed, breaking read state synchronization across clients. Users cannot see which messages others have read, and private read receipts don't work. The infrastructure exists - we just need to wire it up.

---

## BACKGROUND

**Current Location**: [`packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs`](../packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs):71

**The Problem**:
```rust
// Note: m.read and m.read.private are read receipts, which should be handled
// by the receipts endpoint. For now, we acknowledge but don't process them.
if payload.read.is_some() || payload.read_private.is_some() {
    info!("Read receipts provided but will be handled by receipts endpoint");
}
```

**Matrix Spec Requirement**: `POST /_matrix/client/v3/rooms/{roomId}/read_markers` must handle:
- `m.fully_read` - Fully read marker (may already be implemented)
- `m.read` - Public read receipt (visible to all room members)
- `m.read.private` - Private read receipt (only visible to user)

**Repository Available**: `ReceiptRepository` already has complete implementation in [`packages/surrealdb/src/repository/receipt.rs`](../packages/surrealdb/src/repository/receipt.rs)

---

## SUBTASK 1: Verify AppState Has ReceiptRepository

**WHAT**: Ensure `ReceiptRepository` is available in the endpoint handler.

**WHERE**: 
1. Check [`packages/server/src/state.rs`](../packages/server/src/state.rs) for AppState definition
2. Check [`packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs`](../packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs) handler signature

**CURRENT HANDLER** (approximately):
```rust
pub async fn set_read_markers(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Json(payload): Json<ReadMarkersRequest>,
) -> Result<Json<Value>, MatrixError> {
    // ...
}
```

**CHECK AppState**:
```rust
pub struct AppState {
    pub db: Surreal<Any>,
    pub session_service: SessionService,
    pub homeserver_name: String,
    pub receipt_repo: ReceiptRepository,  // CHECK IF THIS EXISTS
    // ... other fields
}
```

**IF MISSING**, add to AppState:
```rust
pub struct AppState {
    pub db: Surreal<Any>,
    pub session_service: SessionService,
    pub homeserver_name: String,
    pub receipt_repo: ReceiptRepository,  // ADD THIS
    // ... other fields
}
```

**AND** initialize in constructor:
```rust
impl AppState {
    pub fn new(db: Surreal<Any>, homeserver_name: String) -> Self {
        Self {
            db: db.clone(),
            session_service: SessionService::new(db.clone()),
            homeserver_name: homeserver_name.clone(),
            receipt_repo: ReceiptRepository::new(db.clone()),  // ADD THIS
            // ... other fields
        }
    }
}
```

**IMPORTS NEEDED** (in state.rs):
```rust
use matryx_surrealdb::repository::receipt::ReceiptRepository;
```

**DEFINITION OF DONE**:
- ✅ ReceiptRepository available in AppState
- ✅ Properly initialized in constructor
- ✅ Imports added

---

## SUBTASK 2: Extract User ID from Authenticated Session

**WHAT**: Get the authenticated user's ID to associate with the receipt.

**WHERE**: [`packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs`](../packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs) handler

**HOW**: Extract user_id from the session or authentication context:

```rust
pub async fn set_read_markers(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Extension(session): Extension<Session>,  // May need to add this
    Json(payload): Json<ReadMarkersRequest>,
) -> Result<Json<Value>, MatrixError> {
    // Extract user_id from session
    let user_id = session.user_id.as_str();
    
    // ... rest of implementation
}
```

**ALTERNATIVE** (if session is stored differently):
```rust
// If user_id is in state or other extension
let user_id = state.session_service.get_current_user_id()?;
```

**CHECK**: Look at other endpoints in the same directory to see how they get user_id.

**DEFINITION OF DONE**:
- ✅ User ID extracted from authenticated session
- ✅ Authentication required for this endpoint
- ✅ Proper error returned if not authenticated

---

## SUBTASK 3: Store m.read Public Receipt

**WHAT**: Store public read receipt when `payload.read` is present.

**WHERE**: Same handler, replace the logging code

**CURRENT CODE**:
```rust
if payload.read.is_some() || payload.read_private.is_some() {
    info!("Read receipts provided but will be handled by receipts endpoint");
}
```

**REPLACE WITH**:
```rust
// Store public read receipt (m.read)
if let Some(read_event_id) = &payload.read {
    state.receipt_repo
        .store_receipt(
            &room_id,
            user_id,
            read_event_id,
            "m.read",              // Public receipt type
            None,                  // thread_id (None for main timeline)
            &state.homeserver_name,
        )
        .await
        .map_err(|e| MatrixError::InternalServerError {
            message: format!("Failed to store read receipt: {}", e),
        })?;
}
```

**Repository Method**:
- `ReceiptRepository::store_receipt(room_id, user_id, event_id, receipt_type, thread_id, server_name)`
- Defined in: [`packages/surrealdb/src/repository/receipt.rs`](../packages/surrealdb/src/repository/receipt.rs):29-88

**DEFINITION OF DONE**:
- ✅ Public read receipt stored when payload.read is present
- ✅ Correct parameters passed to repository
- ✅ Errors handled appropriately
- ✅ Receipt visible to all room members per spec

---

## SUBTASK 4: Store m.read.private Private Receipt

**WHAT**: Store private read receipt when `payload.read_private` is present.

**WHERE**: Same handler, add after m.read handling

**ADD**:
```rust
// Store private read receipt (m.read.private)
if let Some(read_private_event_id) = &payload.read_private {
    state.receipt_repo
        .store_receipt(
            &room_id,
            user_id,
            read_private_event_id,
            "m.read.private",      // Private receipt type
            None,                  // thread_id (None for main timeline)
            &state.homeserver_name,
        )
        .await
        .map_err(|e| MatrixError::InternalServerError {
            message: format!("Failed to store private read receipt: {}", e),
        })?;
}
```

**KEY DIFFERENCE**: `"m.read.private"` receipt type means:
- Only visible to the user who sent it
- Not included in sync for other users
- Used for privacy-conscious clients

**DEFINITION OF DONE**:
- ✅ Private read receipt stored when payload.read_private is present
- ✅ Receipt marked as private in database
- ✅ Only accessible by the user who created it

---

## SUBTASK 5: Handle Thread-Specific Receipts (Optional Enhancement)

**WHAT**: Support threaded read receipts if the payload includes thread_id.

**WHERE**: Same handler

**CHECK PAYLOAD**: Look at `ReadMarkersRequest` structure to see if it has thread support:
```rust
pub struct ReadMarkersRequest {
    pub fully_read: Option<String>,
    pub read: Option<String>,
    pub read_private: Option<String>,
    pub thread_id: Option<String>,  // CHECK IF THIS EXISTS
}
```

**IF SUPPORTED**:
```rust
// Extract thread_id from payload
let thread_id = payload.thread_id.as_deref();

// Pass to store_receipt
state.receipt_repo
    .store_receipt(
        &room_id,
        user_id,
        read_event_id,
        "m.read",
        thread_id,  // Use actual thread_id if present
        &state.homeserver_name,
    )
    .await?;
```

**IF NOT**: Leave as `None` - this is fine for initial implementation. Threads are an advanced feature.

**DEFINITION OF DONE**:
- ✅ Thread support added if payload has thread_id field
- ✅ Falls back to None for main timeline
- ✅ Per-thread read state tracked correctly

---

## SUBTASK 6: Remove Placeholder Code and Add Logging

**WHAT**: Clean up the placeholder comments and add proper logging.

**WHERE**: Same handler

**REMOVE**:
```rust
// Note: m.read and m.read.private are read receipts, which should be handled
// by the receipts endpoint. For now, we acknowledge but don't process them.
if payload.read.is_some() || payload.read_private.is_some() {
    info!("Read receipts provided but will be handled by receipts endpoint");
}
```

**ADD** (after implementation):
```rust
// Log receipt storage for debugging
if payload.read.is_some() {
    tracing::debug!("Stored public read receipt for user {} in room {}", user_id, room_id);
}
if payload.read_private.is_some() {
    tracing::debug!("Stored private read receipt for user {} in room {}", user_id, room_id);
}
```

**DEFINITION OF DONE**:
- ✅ Placeholder comments removed
- ✅ Proper logging added for debugging
- ✅ No "TODO" or "for now" comments remain

---

## SUBTASK 7: Verify Complete Handler

**WHAT**: Ensure the full handler works correctly.

**WHERE**: Final review of [`read_markers.rs`](../packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs)

**COMPLETE HANDLER SHOULD LOOK LIKE**:
```rust
pub async fn set_read_markers(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Extension(session): Extension<Session>,
    Json(payload): Json<ReadMarkersRequest>,
) -> Result<Json<Value>, MatrixError> {
    let user_id = &session.user_id;
    
    // Handle fully_read marker (may already exist)
    if let Some(fully_read_event_id) = &payload.fully_read {
        // ... existing fully_read logic ...
    }
    
    // Store public read receipt
    if let Some(read_event_id) = &payload.read {
        state.receipt_repo
            .store_receipt(
                &room_id,
                user_id,
                read_event_id,
                "m.read",
                None,
                &state.homeserver_name,
            )
            .await?;
        tracing::debug!("Stored public read receipt for user {} in room {}", user_id, room_id);
    }
    
    // Store private read receipt
    if let Some(read_private_event_id) = &payload.read_private {
        state.receipt_repo
            .store_receipt(
                &room_id,
                user_id,
                read_private_event_id,
                "m.read.private",
                None,
                &state.homeserver_name,
            )
            .await?;
        tracing::debug!("Stored private read receipt for user {} in room {}", user_id, room_id);
    }
    
    Ok(Json(json!({})))
}
```

**DEFINITION OF DONE**:
- ✅ Handler accepts and processes all receipt types
- ✅ Error handling is appropriate
- ✅ Returns correct response format
- ✅ Code compiles without errors

---

## SUBTASK 8: Build and Verify

**WHAT**: Ensure code compiles and integrates correctly.

**WHERE**: Run from workspace root

**HOW**:
```bash
# Build server package
cargo build --package matryx_server

# Check for errors
cargo check --package matryx_server

# Build in release mode to verify
cargo build --package matryx_server --release
```

**VERIFY**: No compilation errors, all imports resolved.

**DEFINITION OF DONE**:
- ✅ Code compiles successfully
- ✅ No warnings about unused imports
- ✅ Receipt storage integrated into server

---

## RESEARCH NOTES

### ReceiptRepository API
Location: [`packages/surrealdb/src/repository/receipt.rs`](../packages/surrealdb/src/repository/receipt.rs)

Key method:
```rust
pub async fn store_receipt(
    &self,
    room_id: &str,        // Room containing the event
    user_id: &str,        // User who read the event
    event_id: &str,       // Event that was read
    receipt_type: &str,   // "m.read" or "m.read.private"
    thread_id: Option<&str>,  // Thread ID (None for main timeline)
    server_name: &str,    // Homeserver name
) -> Result<(), RepositoryError>
```

The repository handles:
- UPSERT logic (updates if exists, creates if new)
- Timestamp management
- Private vs public flag setting
- Thread support

### Matrix Specification
- **Endpoint**: `POST /_matrix/client/v3/rooms/{roomId}/read_markers`
- **Spec Reference**: Client-Server API
- **Read Receipt Types**:
  - `m.fully_read` - User's read-up-to position
  - `m.read` - Public read receipt
  - `m.read.private` - Private read receipt

**Request Body**:
```json
{
  "m.fully_read": "$event_id",
  "m.read": "$event_id",
  "m.read.private": "$event_id"
}
```

**Response**: Empty JSON object `{}`

### Privacy Considerations
- `m.read` receipts are broadcast to all room members
- `m.read.private` receipts are ONLY visible to the user who sent them
- Clients can choose which type to use based on user privacy preferences

---

## DEFINITION OF DONE

**Task complete when**:
- ✅ ReceiptRepository available in AppState
- ✅ User ID extracted from authenticated session
- ✅ Public read receipts (`m.read`) stored correctly
- ✅ Private read receipts (`m.read.private`) stored correctly
- ✅ Placeholder code and comments removed
- ✅ Proper error handling and logging added
- ✅ Code compiles successfully
- ✅ Read state synchronization works per Matrix spec

**NO REQUIREMENTS FOR**:
- ❌ Unit tests
- ❌ Integration tests
- ❌ Benchmarks
- ❌ Documentation (beyond code comments)

---

## RELATED FILES

- [`packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs`](../packages/server/src/_matrix/client/v3/rooms/by_room_id/read_markers.rs) - Main file to modify (line 71)
- [`packages/server/src/state.rs`](../packages/server/src/state.rs) - AppState definition
- [`packages/surrealdb/src/repository/receipt.rs`](../packages/surrealdb/src/repository/receipt.rs) - Receipt repository API
- [`./spec/client/03_messaging_communication.md`](../spec/client/03_messaging_communication.md) - Matrix read receipt spec
