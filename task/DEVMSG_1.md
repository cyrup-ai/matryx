# DEVMSG_1: To-Device Message Subscription - COMPLETED

**Status**: ✅ COMPLETE  
**Priority**: HIGH  
**Completion Date**: 2025-10-10

---

## MATRIX SPEC COMPLIANCE

This implementation complies with:
- **Client-Server API**: [Send-to-Device Messaging](../spec/client/04_security_encryption.md#send-to-device-messaging)
- **Server-Server API**: [Send-to-Device Messaging](../spec/server/18-send-to-device.md)

---

## OVERVIEW

### What is To-Device Messaging?

To-device messaging is a critical Matrix protocol feature that enables direct device-to-device communication **outside of room timelines**. Unlike regular Matrix events that are stored in room history, to-device messages:

- Are delivered **exactly once** to each target device
- **Do not persist** in shared communication history
- Are used primarily for **end-to-end encryption** (E2EE) signaling
- Support **wildcard delivery** (`device_id: "*"`) to all user devices

### Primary Use Cases

1. **Encryption Key Distribution**: Exchanging Olm/Megolm session keys between devices
2. **Device Verification**: Interactive SAS (Short Authentication String) verification flows
3. **Key Requests**: Requesting encryption keys from other devices for message decryption
4. **Ephemeral Signaling**: Any temporary device-specific communication

### Matrix Spec Requirements

**Client Behavior** (per spec):
- Send messages via `PUT /_matrix/client/v3/sendToDevice/{eventType}/{txnId}`
- Receive messages in `/sync` response under `to_device.events`
- Messages are **automatically acknowledged** on next `/sync` with `next_batch` token

**Server Behavior** (per spec):
- Store pending messages until delivered
- Return up to 100 messages per `/sync` response
- Delete messages after client acknowledges (next `/sync` call)
- Support federation for cross-server to-device messaging

---

## ARCHITECTURE

### Implementation Stack

This feature implements real-time to-device message delivery using SurrealDB LIVE queries, bypassing traditional polling-based `/sync` for instant delivery.

#### Layer 1: Database Repository
**File**: [`packages/surrealdb/src/repository/to_device.rs`](../packages/surrealdb/src/repository/to_device.rs)

```rust
pub struct ToDeviceRepository {
    db: Surreal<Any>,
}

impl ToDeviceRepository {
    /// Subscribe to to-device messages using SurrealDB LIVE query
    pub async fn subscribe_to_device_messages(
        &self,
        user_id: &str,
        device_id: &str,
    ) -> Result<impl Stream<Item = Result<ToDeviceMessage, RepositoryError>>, RepositoryError> {
        // LIVE query monitors database for real-time changes
        let mut stream = self
            .db
            .query("LIVE SELECT * FROM to_device_messages 
                    WHERE recipient_id = $user_id 
                    AND device_id = $device_id 
                    AND is_delivered = false")
            .bind(("user_id", user_id.to_string()))
            .bind(("device_id", device_id.to_string()))
            .await?;

        // Transform SurrealDB notifications to ToDeviceMessage structs
        Ok(stream.stream::<Notification<Value>>(0)?.map(|notification| {
            // Handle CREATE/UPDATE/DELETE actions
            Self::convert_notification_to_message_result(notification.data)
        }))
    }

    /// Mark messages as delivered (acknowledgment)
    pub async fn mark_to_device_messages_delivered(
        &self,
        user_id: &str,
        device_id: &str,
        message_ids: &[String],
    ) -> Result<(), RepositoryError> {
        // Update is_delivered flag and set delivered_at timestamp
    }
}
```

**Key Methods**:
- `send_to_device()` - Insert messages into database
- `get_to_device_messages()` - Polling-based retrieval (fallback)
- `subscribe_to_device_messages()` - **LIVE query stream** (primary method)
- `mark_to_device_messages_delivered()` - Acknowledgment
- `cleanup_delivered_messages()` - Garbage collection
- `validate_to_device_permissions()` - Security checks (shared room requirement)

**Database Schema**:
```sql
CREATE to_device_messages SET
    message_id = $message_id,        -- Unique identifier
    sender_id = $sender_id,          -- User who sent the message
    recipient_id = $recipient_id,    -- Target user
    device_id = $device_id,          -- Target device (or "*" for all)
    event_type = $event_type,        -- m.room_key, m.room_key_request, etc.
    content = $content,              -- Message payload (JSON)
    is_delivered = false,            -- Delivery status
    created_at = $created_at,        -- Timestamp
    delivered_at = NULL              -- Set on acknowledgment
```

#### Layer 2: Client Service Wrapper
**File**: [`packages/client/src/repositories/client_service.rs`](../packages/client/src/repositories/client_service.rs)

```rust
pub struct ClientRepositoryService {
    to_device_repo: ToDeviceRepository,
    user_id: String,     // ← Context from credentials
    device_id: String,   // ← Context from credentials (THIS WAS ADDED)
    // ... other repos
}

impl ClientRepositoryService {
    /// Create from database with user and device context
    pub fn from_db(db: Surreal<Any>, user_id: String, device_id: String) -> Self {
        // ⚠️ SIGNATURE CHANGED: device_id parameter added
        let to_device_repo = ToDeviceRepository::new(db.clone());
        // ... initialize other repos
        
        Self {
            to_device_repo,
            user_id,
            device_id,  // ← New field
            // ...
        }
    }

    /// Subscribe with implicit user_id and device_id from service context
    pub async fn subscribe_to_device_messages(&self) 
        -> Result<Pin<Box<dyn Stream<Item = Result<ToDeviceMessage, ClientError>>>>, ClientError> 
    {
        tracing::debug!(
            "Subscribing to to-device messages for user {} device {}",
            self.user_id,
            self.device_id
        );

        let stream = self
            .to_device_repo
            .subscribe_to_device_messages(&self.user_id, &self.device_id)
            .await?
            .map(|result| result.map_err(ClientError::Repository));

        Ok(Box::pin(stream))
    }

    /// Acknowledge received messages
    pub async fn acknowledge_to_device_messages(
        &self,
        message_ids: &[String],
    ) -> Result<(), ClientError> {
        self.to_device_repo
            .mark_to_device_messages_delivered(&self.user_id, &self.device_id, message_ids)
            .await?;
        Ok(())
    }
}
```

**Purpose**: Wraps repository methods with authenticated user/device context, eliminating the need to pass these parameters repeatedly.

#### Layer 3: LiveQuery Sync Manager
**File**: [`packages/client/src/sync.rs`](../packages/client/src/sync.rs)

```rust
impl LiveQuerySync {
    /// Constructor now requires device_id
    pub fn new(
        user_id: String,
        device_id: String,  // ← PARAMETER ADDED
        db: Surreal<Any>
    ) -> Self {
        let repository_service = ClientRepositoryService::from_db(
            db,
            user_id.clone(),
            device_id  // ← PASS TO SERVICE
        );
        // ...
    }

    /// Start device subscriptions (spawns background task)
    async fn start_device_subscriptions(&self) -> Result<()> {
        let repository_service = self.repository_service.clone();
        let update_sender = self.update_sender.clone();

        tokio::spawn(async move {
            match repository_service.subscribe_to_device_messages().await {
                Ok(mut stream) => {
                    info!("Subscribed to to-device messages");

                    while let Some(notification_result) = stream.next().await {
                        match notification_result {
                            Ok(to_device_msg) => {
                                debug!("Received to-device message: {:?}", to_device_msg);

                                // Convert to SyncUpdate for broadcast
                                let content = serde_json::to_value(to_device_msg).unwrap_or_default();
                                let update = SyncUpdate::AccountDataUpdate {
                                    data_type: "m.to_device".to_string(),
                                    content,
                                };

                                if let Err(e) = update_sender.send(update) {
                                    warn!("Failed to send to-device message update: {}", e);
                                }
                            },
                            Err(e) => error!("Error in to-device stream: {}", e),
                        }
                    }
                },
                Err(e) => warn!("Could not subscribe to to-device messages: {}", e),
            }
        });

        Ok(())
    }
}
```

**Integration**: Called from `start()` which also initiates:
- `start_event_subscriptions()` - Room timeline events
- `start_membership_subscriptions()` - Room membership changes
- `start_presence_subscriptions()` - User presence updates
- `start_device_subscriptions()` - **To-device messages + device key updates**

#### Layer 4: Realtime Matrix Client
**File**: [`packages/client/src/realtime.rs`](../packages/client/src/realtime.rs)

```rust
impl RealtimeMatrixClient {
    async fn initialize_db(&mut self) -> Result<()> {
        let credentials = self.credentials.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No credentials available"))?;

        // Create service with both user_id and device_id
        self.repository_service = Some(ClientRepositoryService::from_db(
            db,
            credentials.user_id.clone(),
            credentials.device_id.clone()  // ← FIX: Pass device_id
        ));

        Ok(())
    }

    async fn initialize_sync(&mut self) -> Result<()> {
        let credentials = self.credentials.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No credentials available"))?;

        let sync_manager = LiveQuerySync::new(
            credentials.user_id.clone(),
            credentials.device_id.clone(),  // ← FIX: Pass device_id
            db.clone()
        );

        sync_manager.start().await?;
        self.sync_manager = Some(sync_manager);

        Ok(())
    }
}
```

**Credentials Structure**:
```rust
pub struct RealtimeCredentials {
    pub user_id: String,       // e.g., "@alice:example.com"
    pub access_token: String,  // JWT or opaque token
    pub device_id: String,     // e.g., "ABCDEFGHIJ"
}
```

---

## THE BUG AND FIXES

### Root Cause

When `ClientRepositoryService::from_db()` signature was changed from:
```rust
// OLD (2 parameters)
pub fn from_db(db: Surreal<Any>, user_id: String) -> Self
```

To:
```rust
// NEW (3 parameters)
pub fn from_db(db: Surreal<Any>, user_id: String, device_id: String) -> Self
```

...all call sites needed updating, but two were missed:
1. `packages/client/src/realtime.rs:222` - Direct instantiation
2. `packages/client/src/sync.rs:171` - Via `LiveQuerySync::new()`

### Compilation Errors (Now Fixed)

```
error[E0061]: this function takes 3 arguments but 2 arguments were supplied
   --> packages/client/src/realtime.rs:222
   |
222 |         self.repository_service = Some(ClientRepositoryService::from_db(db, user_id));
   |                                        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^------------- 
   |                                                                        argument #3 of type `String` is missing

error[E0061]: this function takes 3 arguments but 2 arguments were supplied
   --> packages/client/src/sync.rs:171
   |
171 |         let repository_service = ClientRepositoryService::from_db(db, user_id.clone());
   |                                  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^--------------------- 
   |                                                                  argument #3 of type `String` is missing
```

### Applied Fixes

#### Fix 1: Update realtime.rs Direct Call
**File**: `packages/client/src/realtime.rs:218-222`

```rust
// BEFORE
let user_id = self.credentials.as_ref()?.user_id.clone();
self.repository_service = Some(ClientRepositoryService::from_db(db, user_id));

// AFTER
let credentials = self.credentials.as_ref()?;
self.repository_service = Some(ClientRepositoryService::from_db(
    db,
    credentials.user_id.clone(),
    credentials.device_id.clone()  // ✅ ADDED
));
```

#### Fix 2: Update LiveQuerySync Constructor
**File**: `packages/client/src/sync.rs:167-178`

```rust
// BEFORE
pub fn new(user_id: String, db: Surreal<Any>) -> Self {
    let repository_service = ClientRepositoryService::from_db(db, user_id.clone());
    // ...
}

// AFTER
pub fn new(
    user_id: String,
    device_id: String,  // ✅ ADDED PARAMETER
    db: Surreal<Any>
) -> Self {
    let repository_service = ClientRepositoryService::from_db(
        db,
        user_id.clone(),
        device_id  // ✅ PASS TO SERVICE
    );
    // ...
}
```

#### Fix 3: Update realtime.rs Sync Initialization
**File**: `packages/client/src/realtime.rs:244-248`

```rust
// BEFORE
let sync_manager = LiveQuerySync::new(
    credentials.user_id.clone(),
    db.clone()
);

// AFTER
let sync_manager = LiveQuerySync::new(
    credentials.user_id.clone(),
    credentials.device_id.clone(),  // ✅ ADDED
    db.clone()
);
```

#### Fix 4: Update Test Code
**File**: `packages/client/src/sync.rs:613, 625`

```rust
// BEFORE
let sync = LiveQuerySync::new("@test:example.com".to_string(), db);

// AFTER
let sync = LiveQuerySync::new(
    "@test:example.com".to_string(),
    "TESTDEVICE".to_string(),  // ✅ ADDED
    db
);
```

---

## VERIFICATION

### Compilation Check
```bash
cd /Volumes/samsung_t9/maxtryx && cargo build -p matryx_client
```

**Result**: ✅ **SUCCESS** - Clean compilation with no errors

```
warning: struct `StreamPosition` is never constructed
   --> packages/surrealdb/src/repository/sync.rs:253:8

warning: method `get_current_stream_id` is never used
    --> packages/surrealdb/src/repository/sync.rs:1005:14

warning: `matryx_surrealdb` (lib) generated 2 warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.13s
```

(Warnings are unrelated to this task and pre-existing)

---

## DEFINITION OF DONE

- [x] All 4 code fixes applied to source files
- [x] `ClientRepositoryService::from_db()` receives `device_id` parameter at all call sites
- [x] `LiveQuerySync::new()` signature updated with `device_id` parameter
- [x] All `LiveQuerySync::new()` call sites updated (production + tests)
- [x] Code compiles without errors: `cargo build -p matryx_client` succeeds
- [x] No new compilation warnings introduced

---

## RELATED FILES

### Implementation Files
- [`packages/surrealdb/src/repository/to_device.rs`](../packages/surrealdb/src/repository/to_device.rs) - Database layer with LIVE queries
- [`packages/client/src/repositories/client_service.rs`](../packages/client/src/repositories/client_service.rs) - Service wrapper layer
- [`packages/client/src/sync.rs`](../packages/client/src/sync.rs) - LiveQuery sync manager
- [`packages/client/src/realtime.rs`](../packages/client/src/realtime.rs) - Realtime Matrix client

### Specification References
- [`spec/client/04_security_encryption.md`](../spec/client/04_security_encryption.md) - Client-Server send-to-device messaging
- [`spec/server/18-send-to-device.md`](../spec/server/18-send-to-device.md) - Server-Server send-to-device messaging

### Entity Definitions
- [`packages/surrealdb/src/repository/mod.rs`](../packages/surrealdb/src/repository/mod.rs) - Exports `ToDeviceMessage` struct

---

## LESSONS LEARNED

**When changing constructor signatures**:
1. Search for ALL call sites: `grep -r "::from_db\|::new" --include="*.rs"`
2. Consider indirect calls via wrapper constructors (e.g., `LiveQuerySync::new` → `ClientRepositoryService::from_db`)
3. Update test code in same commit to catch errors early
4. Use compiler errors as a checklist - fix systematically

**Search commands for future reference**:
```bash
# Find all ClientRepositoryService usages
rg "ClientRepositoryService::" --type rust

# Find all LiveQuerySync instantiations
rg "LiveQuerySync::new" --type rust

# Find all from_db calls
rg "::from_db\(" --type rust
```
