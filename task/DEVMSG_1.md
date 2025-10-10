# DEVMSG_1: Implement To-Device Message Subscription

**Status**: Ready for Implementation  
**Priority**: HIGH  
**Estimated Effort**: 2-4 hours (drastically reduced from initial estimate)  
**Package**: packages/client

---

## EXECUTIVE SUMMARY

**90% OF THIS FEATURE IS ALREADY IMPLEMENTED.** This is a **wiring task**, not a build-from-scratch task.

The ToDeviceRepository has full LIVE query subscription support, database schema exists, and all necessary infrastructure is in place. The work required is:
1. Add `device_id` field to `ClientRepositoryService`
2. Wire up the existing repository's `subscribe_to_device_messages()` method
3. Fix a critical field name mismatch bug

---

## WHAT'S ALREADY DONE

### ✅ ToDeviceRepository Implementation (COMPLETE)
**Location**: [`packages/surrealdb/src/repository/to_device.rs`](../../packages/surrealdb/src/repository/to_device.rs)

The repository has everything needed:
- `ToDeviceMessage` struct with all required fields (lines 10-20)
- `subscribe_to_device_messages()` with LIVE query implementation (lines 412-443)
- `mark_to_device_messages_delivered()` for acknowledgment (lines 221-247)
- `get_to_device_messages()` for polling fallback (lines 165-218)
- Full permission validation logic
- Error handling and stream processing

### ✅ Database Schema (COMPLETE)
**Location**: [`packages/surrealdb/migrations/matryx.surql`](../../packages/surrealdb/migrations/matryx.surql) (lines 1655-1673)

Table `to_device_messages` is fully defined with:
- All required fields (message_id, sender_id, recipient_id, device_id, event_type, content, etc.)
- Proper permissions (recipient-based access control)
- Auto-populated timestamps
- Database event triggers for sync (lines 2294-2307)

### ✅ Service Integration (PARTIAL)
**Location**: [`packages/client/src/repositories/client_service.rs`](../../packages/client/src/repositories/client_service.rs)

- `ToDeviceRepository` already injected (line 44)
- Constructor accepts repository (line 75)
- `subscribe_to_device_messages()` exists but returns empty stream (line 178)
- TODO comment indicates awaiting repository support (BUT SUPPORT ALREADY EXISTS)

---

## CRITICAL BUG DISCOVERED

### Field Name Mismatch: `event_type` vs `message_type`

**Issue**: The ToDeviceMessage struct uses `event_type` but the database schema defines `message_type`.

**Evidence**:
- **Struct field** ([`to_device.rs:15`](../../packages/surrealdb/src/repository/to_device.rs#L15)): `pub event_type: String`
- **Database field** ([`matryx.surql:1669`](../../packages/surrealdb/migrations/matryx.surql#L1669)): `DEFINE FIELD message_type`
- **Query usage** ([`to_device.rs:135`](../../packages/surrealdb/src/repository/to_device.rs#L135)): Uses `message_type` in INSERT

**Impact**: This mismatch will cause deserialization errors when LIVE queries return data.

**Fix Required**: Standardize on `event_type` (Matrix spec compliant) or add serde rename.

---

## ACTUAL WORK REQUIRED

### TASK 1: Add device_id Field to ClientRepositoryService

**Objective**: Store the device context in the service struct

**File**: [`packages/client/src/repositories/client_service.rs`](../../packages/client/src/repositories/client_service.rs)

**Current Struct** (line 39):
```rust
#[derive(Clone)]
pub struct ClientRepositoryService {
    event_repo: EventRepository,
    membership_repo: MembershipRepository,
    presence_repo: PresenceRepository,
    device_repo: DeviceRepository,
    to_device_repo: ToDeviceRepository,
    user_id: String,
}
```

**Required Change**:
```rust
#[derive(Clone)]
pub struct ClientRepositoryService {
    event_repo: EventRepository,
    membership_repo: MembershipRepository,
    presence_repo: PresenceRepository,
    device_repo: DeviceRepository,
    to_device_repo: ToDeviceRepository,
    user_id: String,
    device_id: String,  // ← ADD THIS
}
```

**Update Constructor** (line 48):
```rust
pub fn new(
    event_repo: EventRepository,
    membership_repo: MembershipRepository,
    presence_repo: PresenceRepository,
    device_repo: DeviceRepository,
    to_device_repo: ToDeviceRepository,
    user_id: String,
    device_id: String,  // ← ADD THIS PARAMETER
) -> Self {
    Self {
        event_repo,
        membership_repo,
        presence_repo,
        device_repo,
        to_device_repo,
        user_id,
        device_id,  // ← ADD THIS FIELD
    }
}
```

**Update from_db** (line 66):
```rust
pub fn from_db(db: Surreal<Any>, user_id: String, device_id: String) -> Self {
    let event_repo = EventRepository::new(db.clone());
    let membership_repo = MembershipRepository::new(db.clone());
    let presence_repo = PresenceRepository::new(db.clone());
    let device_repo = DeviceRepository::new(db.clone());
    let to_device_repo = ToDeviceRepository::new(db);

    Self::new(
        event_repo, 
        membership_repo, 
        presence_repo, 
        device_repo, 
        to_device_repo, 
        user_id, 
        device_id  // ← ADD THIS ARGUMENT
    )
}
```

---

### TASK 2: Wire Up subscribe_to_device_messages

**Objective**: Replace empty stream with actual repository call

**File**: [`packages/client/src/repositories/client_service.rs`](../../packages/client/src/repositories/client_service.rs)

**Current Implementation** (lines 178-190):
```rust
/// Subscribe to to-device messages for the current user
/// TODO: Implement when ToDeviceRepository has subscription support
pub async fn subscribe_to_device_messages<'a>(
    &'a self,
) -> Result<
    Pin<Box<dyn Stream<Item = Result<ToDeviceMessage, ClientError>> + Send + 'a>>,
    ClientError,
> {
    use futures_util::stream;
    // Return empty stream until ToDeviceRepository implements subscriptions
    let stream = stream::empty();
    Ok(Box::pin(stream))
}
```

**Required Implementation**:
```rust
/// Subscribe to to-device messages for the current user
///
/// Creates a SurrealDB LIVE query stream for real-time to-device message delivery.
/// Messages are delivered as they arrive and should be acknowledged after processing.
///
/// # Returns
/// A stream of to-device messages or errors
///
/// # Errors
/// Returns `ClientError` if the subscription cannot be created
pub async fn subscribe_to_device_messages<'a>(
    &'a self,
) -> Result<
    Pin<Box<dyn Stream<Item = Result<ToDeviceMessage, ClientError>> + Send + 'a>>,
    ClientError,
> {
    tracing::debug!(
        "Subscribing to to-device messages for user {} device {}",
        self.user_id,
        self.device_id
    );

    // Call the repository's subscription method
    let stream = self
        .to_device_repo
        .subscribe_to_device_messages(&self.user_id, &self.device_id)
        .await?
        .map(|result| result.map_err(ClientError::Repository));

    Ok(Box::pin(stream))
}
```

**Notes**:
- Remove the TODO comment
- The repository method is at [`to_device.rs:412`](../../packages/surrealdb/src/repository/to_device.rs#L412)
- Error conversion from `RepositoryError` to `ClientError` is handled by `map_err`
- The repository handles LIVE query setup and stream transformation

---

### TASK 3: Add Message Acknowledgment Method (OPTIONAL)

**Objective**: Provide helper for marking messages as delivered

**File**: [`packages/client/src/repositories/client_service.rs`](../../packages/client/src/repositories/client_service.rs)

**Add After subscribe_to_device_messages**:
```rust
/// Mark to-device messages as delivered
///
/// Should be called after successfully processing to-device messages.
/// This allows the server to clean up delivered messages.
///
/// # Arguments
/// * `message_ids` - List of message IDs to acknowledge
///
/// # Errors
/// Returns `ClientError` if the acknowledgment fails
pub async fn acknowledge_to_device_messages(
    &self,
    message_ids: &[String],
) -> Result<(), ClientError> {
    tracing::debug!("Acknowledging {} to-device messages", message_ids.len());

    self.to_device_repo
        .mark_to_device_messages_delivered(&self.user_id, &self.device_id, message_ids)
        .await?;

    Ok(())
}
```

**Why Optional**: The repository method already exists and works. This is just a convenience wrapper.

---

### TASK 4: Fix Field Name Mismatch

**Objective**: Resolve `event_type` vs `message_type` inconsistency

**Option A**: Rename database field (requires migration)
**Option B**: Add serde rename to struct (simple, no migration)

**Recommended: Option B**

**File**: [`packages/surrealdb/src/repository/to_device.rs`](../../packages/surrealdb/src/repository/to_device.rs)

**Change** (line 14):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToDeviceMessage {
    pub message_id: String,
    pub sender_id: String,
    pub recipient_id: String,
    pub device_id: String,
    #[serde(rename = "message_type")]  // ← ADD THIS to match DB schema
    pub event_type: String,
    pub content: Value,
    pub txn_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub is_delivered: bool,
}
```

**Why**: Database schema uses `message_type`, but Matrix spec uses `event_type`. The serde rename bridges this gap.

---

## ARCHITECTURE NOTES

### Two ToDeviceMessage Types (This is Correct)

1. **Entity Type**: [`packages/entity/src/types/to_device_message.rs`](../../packages/entity/src/types/to_device_message.rs)
   - Used for Matrix API requests/responses
   - Contains `HashMap<String, HashMap<String, EventContent>>`
   - Represents the `/sendToDevice` API format

2. **Repository Type**: [`packages/surrealdb/src/repository/to_device.rs`](../../packages/surrealdb/src/repository/to_device.rs)
   - Used for database storage and subscriptions
   - Represents individual stored messages
   - Used by LIVE queries and sync operations

**They serve different purposes and both are needed.**

### How LIVE Queries Work

The repository's `subscribe_to_device_messages()` uses SurrealDB's LIVE SELECT:
```sql
LIVE SELECT * FROM to_device_messages 
WHERE recipient_id = $user_id 
  AND device_id = $device_id 
  AND is_delivered = false
```

This returns a stream that:
1. Delivers all existing undelivered messages immediately
2. Pushes new messages as they arrive in real-time
3. Continues until the client disconnects or cancels

**Reference**: [`to_device.rs:420-427`](../../packages/surrealdb/src/repository/to_device.rs#L420)

### Message Flow

```
┌─────────────┐                 ┌──────────────┐                ┌─────────────────┐
│   Server    │                 │  SurrealDB   │                │     Client      │
│     API     │                 │   Database   │                │    Service      │
└──────┬──────┘                 └──────┬───────┘                └────────┬────────┘
       │                                │                                 │
       │  INSERT to_device_messages     │                                 │
       │───────────────────────────────>│                                 │
       │                                │                                 │
       │                                │  LIVE query notification        │
       │                                │────────────────────────────────>│
       │                                │                                 │
       │                                │  Client processes message       │
       │                                │                                 │
       │                                │  acknowledge_to_device_messages │
       │                                │<────────────────────────────────│
       │                                │                                 │
       │  UPDATE is_delivered = true    │                                 │
       │───────────────────────────────>│                                 │
```

---

## DEFINITION OF DONE

- [x] ToDeviceRepository exists with LIVE query support (ALREADY DONE)
- [x] Database schema created (ALREADY DONE)
- [ ] `device_id` field added to `ClientRepositoryService`
- [ ] Constructors updated to accept `device_id`
- [ ] `subscribe_to_device_messages()` wired to repository
- [ ] TODO comment removed
- [ ] Field name mismatch resolved with serde rename
- [ ] Optional: Acknowledgment helper method added
- [ ] No compilation errors
- [ ] Code compiles and type-checks

---

## FILES TO MODIFY

**Primary Changes**:
1. [`packages/client/src/repositories/client_service.rs`](../../packages/client/src/repositories/client_service.rs)
   - Lines 39-64: Add `device_id` field, update constructors
   - Lines 178-190: Replace empty stream with repository call
   - Add acknowledgment method after line 190

**Bug Fix**:
2. [`packages/surrealdb/src/repository/to_device.rs`](../../packages/surrealdb/src/repository/to_device.rs)
   - Line 14: Add `#[serde(rename = "message_type")]` to `event_type` field

**No New Files Required** - Everything already exists!

---

## REFERENCES

### Existing Code
- **ToDeviceRepository**: [`packages/surrealdb/src/repository/to_device.rs`](../../packages/surrealdb/src/repository/to_device.rs)
- **ClientRepositoryService**: [`packages/client/src/repositories/client_service.rs`](../../packages/client/src/repositories/client_service.rs)
- **Database Schema**: [`packages/surrealdb/migrations/matryx.surql:1655-1673`](../../packages/surrealdb/migrations/matryx.surql)
- **Entity Type**: [`packages/entity/src/types/to_device_message.rs`](../../packages/entity/src/types/to_device_message.rs)

### Matrix Specification
- **To-Device Messaging**: Matrix Client-Server API, Section on End-to-End Encryption
- **Message Types**: `m.room_key`, `m.room_key_request`, `m.key.verification.*`
- **Delivery**: Messages must be delivered exactly once per device and deleted after acknowledgment

### SurrealDB Documentation
- **LIVE SELECT**: Real-time query subscriptions
- **Streams**: Async stream processing with futures

---

## NOTES

### Why This Task Looked Bigger Than It Is

The original task specification was written without knowledge that:
1. ToDeviceRepository was already fully implemented
2. Database schema was already created
3. Service integration was 90% complete
4. Only wiring was needed, not building

### Why 2-4 Hours (Not 1 Week)

The work is:
- Add 1 field to a struct
- Update 2 constructors
- Replace 3 lines in one method
- Add 1 serde annotation
- Optional: Add 1 helper method

This is a **small wiring task**, not a feature implementation.

### Critical Success Factor

**Fix the field name mismatch** or the LIVE query will fail to deserialize messages, making the entire subscription non-functional.

---

## EXAMPLE USAGE

After implementation, clients can subscribe like this:

```rust
let service = ClientRepositoryService::from_db(
    db, 
    "@alice:example.com".to_string(),
    "DEVICEABC123".to_string()  // ← Now required
);

// Subscribe to messages
let mut stream = service.subscribe_to_device_messages().await?;

// Process messages as they arrive
while let Some(result) = stream.next().await {
    match result {
        Ok(message) => {
            println!("Received: {} from {}", message.event_type, message.sender_id);
            
            // Process the message (e.g., decrypt, handle key exchange)
            handle_message(&message).await?;
            
            // Acknowledge delivery
            service.acknowledge_to_device_messages(&[message.message_id]).await?;
        }
        Err(e) => eprintln!("Stream error: {}", e),
    }
}
```
