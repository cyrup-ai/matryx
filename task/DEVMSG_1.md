# DEVMSG_1: Implement To-Device Message Subscription

**Status**: Ready for Implementation
**Priority**: HIGH
**Estimated Effort**: 1 week
**Package**: packages/client

---

## OBJECTIVE

Implement to-device message subscription using SurrealDB LIVE queries to enable end-to-end encryption device messaging, key verification, and device-to-device communication.

---

## PROBLEM DESCRIPTION

The client service has a TODO marker for to-device message subscription:

File: `packages/client/src/repositories/client_service.rs:163-168`
```rust
/// Subscribe to to-device messages for the current user
// TODO: Implement to-device message subscription
pub async fn subscribe_to_device_messages<'a>(
    &'a self,
) -> Result<
    Pin<Box<dyn Stream<Item = Result<ToDeviceMessage, ClientError>> + Send + 'a>>,
    ClientError
> {
```

Without this implementation:
- End-to-end encryption device messaging is non-functional
- Key verification messages cannot be received
- Device-to-device communication is broken
- Cross-device encryption setup fails

---

## RESEARCH NOTES

**Matrix Specification**:
- To-device messages are ephemeral messages sent directly to specific devices
- Used for E2EE key exchange (m.room_key, m.room_key_request)
- Used for device verification (m.key.verification.*)
- Must be delivered exactly once per device
- Should be deleted after delivery to client

**SurrealDB LIVE Queries**:
- Real-time subscription to table changes
- Syntax: `LIVE SELECT * FROM table WHERE condition`
- Returns a stream of changes
- Ideal for push-based message delivery

**To-Device Message Flow**:
1. Server receives to-device message via sync or dedicated endpoint
2. Server stores in to_device_messages table
3. Client subscribes to LIVE query filtered by recipient device
4. Client receives messages in real-time
5. Client acknowledges delivery, server deletes message

---

## SUBTASK 1: Define To-Device Message Schema

**Objective**: Create the database schema and Rust types for to-device messages.

**Location**: `packages/entity/src/types/` (create new file if needed)

**Implementation**:

1. Create `ToDeviceMessage` struct if it doesn't exist:
```rust
/// To-device message for device-to-device communication
///
/// Used for E2EE key exchange, device verification, and other
/// device-specific communication per Matrix specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToDeviceMessage {
    /// Unique message identifier
    pub message_id: String,

    /// Message sender (user ID)
    pub sender: String,

    /// Sender's device ID
    pub sender_device_id: String,

    /// Recipient user ID
    pub recipient: String,

    /// Recipient device ID
    pub recipient_device_id: String,

    /// Message type (e.g., "m.room_key", "m.key.verification.request")
    #[serde(rename = "type")]
    pub msg_type: String,

    /// Message content (encrypted or plaintext depending on type)
    pub content: serde_json::Value,

    /// Timestamp when message was created (Unix millis)
    pub created_at: i64,
}
```

2. Add error types to `ClientError` enum if needed:
```rust
pub enum ClientError {
    // ... existing variants ...

    /// Not authenticated (no access token)
    NotAuthenticated,

    /// Database error during operation
    DatabaseError(String),

    /// Stream error
    StreamError(String),
}
```

**Files to Create/Modify**:
- `packages/entity/src/types/to_device.rs` (create if doesn't exist)
- `packages/entity/src/types/mod.rs` (add module declaration)
- `packages/client/src/error.rs` (add error variants if needed)

**Definition of Done**:
- ToDeviceMessage struct defined with all required fields
- Proper serde annotations for Matrix compatibility
- ClientError has appropriate variants for this feature
- Documentation explains the purpose and Matrix spec compliance

---

## SUBTASK 2: Implement LIVE Query Subscription

**Objective**: Create the subscription method using SurrealDB LIVE SELECT.

**Location**: `packages/client/src/repositories/client_service.rs`

**Current Stub** (lines 163-175):
```rust
/// Subscribe to to-device messages for the current user
// TODO: Implement to-device message subscription
pub async fn subscribe_to_device_messages<'a>(
    &'a self,
) -> Result<
    Pin<Box<dyn Stream<Item = Result<ToDeviceMessage, ClientError>> + Send + 'a>>,
    ClientError
> {
    // Placeholder - need LIVE query implementation
    Err(ClientError::NotImplemented)
}
```

**Required Implementation**:
```rust
/// Subscribe to to-device messages for the current user
///
/// Creates a SurrealDB LIVE query that streams new to-device messages
/// in real-time as they arrive. Messages should be acknowledged and
/// deleted after successful delivery.
///
/// # Returns
/// A stream of to-device messages or errors
///
/// # Errors
/// - `NotAuthenticated` if no user is logged in
/// - `DatabaseError` if subscription setup fails
pub async fn subscribe_to_device_messages<'a>(
    &'a self,
) -> Result<
    Pin<Box<dyn Stream<Item = Result<ToDeviceMessage, ClientError>> + Send + 'a>>,
    ClientError
> {
    // Verify user is authenticated
    let user_id = self.user_id
        .as_ref()
        .ok_or(ClientError::NotAuthenticated)?
        .clone();

    // Get device ID for this client instance
    let device_id = self.device_id
        .as_ref()
        .ok_or(ClientError::NotAuthenticated)?
        .clone();

    tracing::debug!(
        "Subscribing to to-device messages for user {} device {}",
        user_id,
        device_id
    );

    // Create LIVE query for this specific device
    let query = r#"
        LIVE SELECT * FROM to_device_messages
        WHERE recipient = $user_id
        AND recipient_device_id = $device_id
        ORDER BY created_at ASC
    "#;

    // Execute LIVE query
    let mut result = self.db
        .query(query)
        .bind(("user_id", user_id.clone()))
        .bind(("device_id", device_id.clone()))
        .await
        .map_err(|e| ClientError::DatabaseError(format!("Failed to create LIVE query: {}", e)))?;

    // Get the stream from the query result
    let stream = result
        .stream::<ToDeviceMessage>(0)
        .map_err(|e| ClientError::DatabaseError(format!("Failed to get stream: {}", e)))?;

    // Transform stream to handle errors and filter
    let filtered_stream = stream.filter_map(move |result| {
        async move {
            match result {
                Ok(notification) => {
                    // Extract the actual message from the notification
                    // (LIVE queries return a notification wrapper)
                    match notification.data {
                        Some(msg) => Some(Ok(msg)),
                        None => {
                            tracing::warn!("LIVE query notification with no data");
                            None
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Error in to-device message stream: {}", e);
                    Some(Err(ClientError::StreamError(e.to_string())))
                }
            }
        }
    });

    Ok(Box::pin(filtered_stream))
}
```

**Files to Modify**:
- `packages/client/src/repositories/client_service.rs` (lines 163-175)

**Definition of Done**:
- TODO comment removed
- LIVE query properly filters by recipient and device
- Stream returns actual ToDeviceMessage structs
- Error handling for database and stream errors
- Logging for debugging
- No unwrap() or expect() calls

---

## SUBTASK 3: Add Message Acknowledgment Method

**Objective**: Allow clients to mark messages as delivered so they can be deleted.

**Location**: `packages/client/src/repositories/client_service.rs`

**Implementation**:

Add new method to ClientService:
```rust
/// Mark a to-device message as delivered and delete it
///
/// Should be called after successfully processing a to-device message.
/// The message will be permanently deleted from the server.
///
/// # Arguments
/// * `message_id` - The ID of the message to acknowledge
///
/// # Errors
/// - `DatabaseError` if deletion fails
pub async fn acknowledge_to_device_message(
    &self,
    message_id: &str,
) -> Result<(), ClientError> {
    tracing::debug!("Acknowledging to-device message: {}", message_id);

    self.db
        .delete(("to_device_messages", message_id))
        .await
        .map_err(|e| ClientError::DatabaseError(format!(
            "Failed to delete to-device message {}: {}",
            message_id, e
        )))?;

    Ok(())
}
```

**Files to Modify**:
- `packages/client/src/repositories/client_service.rs`

**Definition of Done**:
- Method properly deletes message by ID
- Error handling with descriptive messages
- Logging for debugging
- Documentation explains when to call this method

---

## SUBTASK 4: Add Database Table Definition

**Objective**: Ensure the database schema for to_device_messages exists.

**Location**: `packages/surrealdb/migrations/` or schema initialization code

**Schema Definition**:
```sql
-- To-device messages table for E2EE and device-to-device communication
DEFINE TABLE to_device_messages SCHEMAFULL;

DEFINE FIELD message_id ON to_device_messages TYPE string
    ASSERT $value != NONE AND $value != '';

DEFINE FIELD sender ON to_device_messages TYPE string
    ASSERT $value != NONE AND $value != '';

DEFINE FIELD sender_device_id ON to_device_messages TYPE string
    ASSERT $value != NONE AND $value != '';

DEFINE FIELD recipient ON to_device_messages TYPE string
    ASSERT $value != NONE AND $value != '';

DEFINE FIELD recipient_device_id ON to_device_messages TYPE string
    ASSERT $value != NONE AND $value != '';

DEFINE FIELD msg_type ON to_device_messages TYPE string
    ASSERT $value != NONE AND $value != '';

DEFINE FIELD content ON to_device_messages TYPE object
    ASSERT $value != NONE;

DEFINE FIELD created_at ON to_device_messages TYPE datetime
    ASSERT $value != NONE;

-- Index for efficient querying by recipient and device
DEFINE INDEX recipient_device_idx ON to_device_messages
    FIELDS recipient, recipient_device_id;

-- Index for cleanup by timestamp
DEFINE INDEX created_at_idx ON to_device_messages
    FIELDS created_at;
```

**Migration Script** (if using migrations):
```rust
// In migration file
pub async fn up(db: &Surreal<Any>) -> Result<(), Box<dyn std::error::Error>> {
    db.query(include_str!("create_to_device_messages_table.sql"))
        .await?;
    Ok(())
}
```

**Files to Create/Modify**:
- `packages/surrealdb/migrations/XXX_create_to_device_messages.sql` (create)
- Or schema initialization code in `packages/surrealdb/src/schema.rs`

**Definition of Done**:
- Table schema defined with all required fields
- Indexes created for efficient queries (recipient_device_idx)
- Field constraints ensure data integrity
- Schema compatible with SurrealDB LIVE queries

---

## SUBTASK 5: Update ClientService Struct Fields

**Objective**: Ensure ClientService has the required fields (device_id).

**Location**: `packages/client/src/repositories/client_service.rs`

**Verify/Add Fields**:
```rust
pub struct ClientService {
    db: Surreal<Any>,
    user_id: Option<String>,
    device_id: Option<String>,  // ← Verify this exists
    // ... other fields
}
```

If device_id doesn't exist:
1. Add to struct definition
2. Update constructor to accept/generate device_id
3. Update login/authentication methods to set device_id

**Implementation** (if needed):
```rust
impl ClientService {
    pub fn new(db: Surreal<Any>) -> Self {
        Self {
            db,
            user_id: None,
            device_id: None,
        }
    }

    pub fn set_device_id(&mut self, device_id: String) {
        self.device_id = Some(device_id);
    }

    pub fn device_id(&self) -> Option<&str> {
        self.device_id.as_deref()
    }
}
```

**Files to Modify**:
- `packages/client/src/repositories/client_service.rs`

**Definition of Done**:
- device_id field exists in ClientService struct
- Getter method for device_id
- Setter method or constructor parameter for device_id
- device_id is set during authentication/login

---

## CONSTRAINTS

⚠️ **NO TESTS**: Do not write unit tests, integration tests, or test fixtures. Test team handles all testing.

⚠️ **NO BENCHMARKS**: Do not write benchmark code. Performance team handles benchmarking.

⚠️ **FOCUS ON FUNCTIONALITY**: Only modify production code in ./src directories.

---

## DEPENDENCIES

**SurrealDB**:
- LIVE SELECT query support (available in SurrealDB 1.0+)
- Stream API for real-time results

**Matrix Specification**:
- Clone: https://github.com/matrix-org/matrix-spec
- Section: Client-Server API - To-Device Messaging
- Message types: m.room_key, m.room_key_request, m.key.verification.*

**Rust Crates**:
- futures (for Stream trait)
- tokio-stream (for stream utilities)

---

## DEFINITION OF DONE

- [ ] ToDeviceMessage struct defined with all Matrix-required fields
- [ ] subscribe_to_device_messages() fully implemented with LIVE query
- [ ] TODO comment removed from client_service.rs
- [ ] acknowledge_to_device_message() method added
- [ ] Database schema created for to_device_messages table
- [ ] Indexes created for efficient querying
- [ ] ClientService has device_id field
- [ ] Error handling for all failure cases
- [ ] Logging for debugging
- [ ] No compilation errors
- [ ] No test code written
- [ ] No benchmark code written

---

## FILES TO MODIFY

1. `packages/entity/src/types/to_device.rs` (create)
2. `packages/entity/src/types/mod.rs` (add module)
3. `packages/client/src/repositories/client_service.rs` (lines 163-175 + new method)
4. `packages/client/src/error.rs` (add error variants if needed)
5. `packages/surrealdb/migrations/XXX_create_to_device_messages.sql` (create)

---

## NOTES

- To-device messages are ephemeral - delete after delivery
- LIVE queries provide push-based delivery (better than polling)
- Messages must be device-specific (not just user-specific)
- Consider adding message expiration (auto-delete after N hours)
- This is critical for E2EE functionality
- Stream must be long-lived - client keeps subscription open
