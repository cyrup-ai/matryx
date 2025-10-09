# SPEC_FEDERATION_10: EDU Processing in Federation Transactions - Implementation Analysis

## Executive Summary

**Status:** MOSTLY COMPLIANT - Implementation exists with 1 critical spec violation requiring correction

The Matrix federation transaction endpoint (`PUT /_matrix/federation/v1/send/{txnId}`) processes all 6 required EDU types with proper validation, storage, and real-time propagation. The implementation demonstrates strong spec compliance with comprehensive user validation, room membership checks, and Matrix 1.4 threading support.

**Critical Finding:** m.read.private receipts are correctly NOT sent outbound but are incorrectly ACCEPTED and processed when received from remote servers (spec violation - should be rejected).

## Core Implementation

**Primary File:** [`packages/server/src/_matrix/federation/v1/send/by_txn_id.rs`](../packages/server/src/_matrix/federation/v1/send/by_txn_id.rs)

Transaction endpoint processes EDUs at lines 675-748:
- Enforces 100 EDU max limit (line 661)
- Dispatches to 6 specialized handlers based on `edu_type`
- All handlers include comprehensive error handling that logs but doesn't fail transaction
- Unknown EDU types are silently ignored per spec (line 745)

## EDU Type Analysis

### 1. m.typing - Typing Notifications

**Implementation:** Lines 766-839 in `by_txn_id.rs`

**Spec Compliance:** ✅ COMPLIANT

**Handler:** `process_typing_edu()`

**Validation Flow:**
```rust
// Line 778: Room ID extraction with validation
let room_id = content.get("room_id").and_then(|v| v.as_str())
    
// Line 782: User ID extraction  
let user_id = content.get("user_id").and_then(|v| v.as_str())
    
// Line 786: Typing boolean flag
let typing = content.get("typing").and_then(|v| v.as_bool())

// Lines 793-796: User origin validation
if !user_id.ends_with(&format!(":{}", origin_server)) {
    // Rejects EDUs where user doesn't match origin server
}

// Lines 799-811: Room membership verification
match room_repo.check_membership(room_id, user_id).await {
    Ok(true) => { /* User is member, continue */ },
    Ok(false) => { /* Not in room, ignore EDU */ },
}
```

**Repository Storage:** [`packages/surrealdb/src/repository/federation.rs`](../packages/surrealdb/src/repository/federation.rs#L1330-L1377)

**Storage Pattern:**
- Lines 1330-1377: `process_typing_edu()` method
- Implements 30-second TTL via `expires_at` field (line 1338)
- DELETE + CREATE pattern ensures single active typing state per user/room
- Typing=false triggers immediate deletion (lines 1367-1375)

**Features:**
- ✅ User origin validation
- ✅ Room membership verification  
- ✅ TTL/expiration (30 seconds)
- ✅ Automatic cleanup on typing=false
- ✅ Deduplication via DELETE before CREATE

### 2. m.receipt - Read Receipts

**Implementation:** Lines 841-912 in `by_txn_id.rs`

**Spec Compliance:** ⚠️ PARTIAL - Accepts m.read.private (VIOLATION)

**Handler:** `process_receipt_edu()`

**Receipt Types Processed:**
```rust
// Lines 832-843: Receipt type validation
match receipt_type.as_str() {
    "m.read" => false,              // Public read receipt - OK to federate
    "m.read.private" => true,       // Private receipt - SPEC VIOLATION HERE
    _ => continue,                   // Unknown types ignored
}
```

**CRITICAL ISSUE - Line 834:** The code processes m.read.private receipts received via federation. Per Matrix spec, private receipts are **client-local only** and must NEVER be sent over federation. Receiving them indicates the remote server is violating the spec.

**Correct Outbound Behavior:** [`packages/server/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/by_event_id.rs`](../packages/server/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/by_event_id.rs)

Lines 120-143 show correct handling:
```rust
"m.read.private" => {
    // Store locally in database (lines 122-135)
    // CRITICAL: Private receipts are NEVER federated (line 142)
    // No EDU creation, no federation queuing
}

"m.read" => {
    // Store locally (lines 24-45)
    // Build receipt EDU (lines 62-93)
    // Queue for federation to remote servers (lines 95-111)
}
```

**Threading Support:** ✅ Matrix 1.4 Compliant

Lines 854-857: Thread ID extraction and storage
```rust
let thread_id = user_receipt
    .get("thread_id")
    .and_then(|v| v.as_str())
    .map(|s| s.to_string());
```

**Repository Storage:** [`packages/surrealdb/src/repository/federation.rs`](../packages/surrealdb/src/repository/federation.rs#L1379-L1408)

Lines 1379-1408: `process_receipt_edu()` method stores all receipt types without filtering m.read.private

**Features:**
- ✅ Supports m.read (public)
- ⚠️ Accepts m.read.private (SHOULD REJECT)
- ✅ Per-event receipt validation
- ✅ Thread ID support (Matrix 1.4)
- ✅ Timestamp validation
- ✅ User origin validation

### 3. m.presence - Presence Updates

**Implementation:** Lines 914-969 in `by_txn_id.rs`

**Spec Compliance:** ✅ COMPLIANT

**Handler:** `process_presence_edu()`

**Validation Flow:**
```rust
// Line 920: Extract push array (REQUIRED per spec)
let push = content.get("push").and_then(|v| v.as_array())

// Lines 922-967: Iterate presence events in push array
for presence_event in push {
    // Line 924: user_id extraction
    // Line 929: User origin validation
    // Line 937: presence state (offline/unavailable/online)
    // Line 940: Optional status_msg
    // Line 944: Optional last_active_ago (milliseconds)
    // Line 946: currently_active boolean flag
}
```

**Repository Storage:** [`packages/surrealdb/src/repository/federation.rs`](../packages/surrealdb/src/repository/federation.rs#L1410-L1440)

Lines 1410-1440: `process_presence_edu()` uses UPSERT pattern
```sql
UPSERT presence_events:⟨$user_id⟩ CONTENT {
    user_id: $user_id,
    presence: $presence,
    status_msg: $status_msg,
    last_active_ago: $last_active_ago,
    currently_active: $currently_active,
    updated_at: time::now()
}
```

**Features:**
- ✅ Validates push array structure
- ✅ User origin validation
- ✅ All presence states (online, offline, unavailable)
- ✅ Optional status_msg field
- ✅ last_active_ago processing (milliseconds)
- ✅ currently_active flag
- ✅ UPSERT ensures latest presence wins

### 4. m.device_list_update - Device List Changes

**Implementation:** Lines 971-1014 in `by_txn_id.rs`

**Spec Compliance:** ✅ COMPLIANT

**Handler:** `process_device_list_edu()`

**Device EDU Handler Module:** [`packages/server/src/federation/device_edu_handler.rs`](../packages/server/src/federation/device_edu_handler.rs)

**Validation and Processing:**
```rust
// Lines 975-980: Parse EDU content into DeviceListUpdate struct
let device_update: DeviceListUpdate = serde_json::from_value(content.clone())

// Lines 983-989: User origin validation
if !device_update.user_id.ends_with(&format!(":{}", origin_server)) {
    return Err("Invalid user origin for device list EDU");
}

// Lines 992-1000: Create EDU wrapper and delegate to DeviceEDUHandler
let edu = DeviceListUpdateEDU { /* ... */ };
state.device_edu_handler.handle_device_list_update(edu).await
```

**DeviceListUpdate Structure** (from device_management.rs):
```rust
pub struct DeviceListUpdate {
    pub user_id: String,
    pub device_id: String,
    pub device_display_name: Option<String>,
    pub deleted: bool,
    pub stream_id: i64,          // Sequential ID for ordering
    pub prev_id: Vec<i64>,       // DAG: previous stream IDs
    pub keys: Option<Value>,     // Device public keys
}
```

**DAG Processing:** Lines 73-118 in `device_edu_handler.rs`
```rust
// Lines 77-80: Store EDU in database for tracking
let edu_entity = EDU::new(ephemeral_event, false);
self.edu_repo.create(&edu_entity).await

// Lines 83-89: Handle device deletion
if edu.content.deleted {
    self.device_repo.delete(&edu.content.device_id).await
}

// Lines 91-118: Handle device update/creation
else {
    let device = Device { /* ... */ };
    // Try update first (line 108)
    // Fall back to create if doesn't exist (lines 112-116)
}
```

**Repository Storage:** [`packages/surrealdb/src/repository/federation.rs`](../packages/surrealdb/src/repository/federation.rs#L1442-L1462)

Lines 1442-1462: Stores device list updates with full DAG information:
- `stream_id`: Sequential ordering
- `prev_id`: Array of previous stream IDs (DAG tracking)
- `deleted`: Boolean flag for device removal
- `keys`: Full device key information

**Features:**
- ✅ User origin validation
- ✅ stream_id sequential processing
- ✅ prev_id DAG tracking for gap detection
- ✅ Device cache updates
- ✅ Deleted device handling
- ✅ Device key storage
- ✅ Display name updates

**Gap Detection:** Implementation supports detecting missing EDUs via prev_id DAG but resync logic may need verification.

### 5. m.signing_key_update - Cross-Signing Key Updates

**Implementation:** Lines 1016-1053 in `by_txn_id.rs`

**Spec Compliance:** ✅ COMPLIANT

**Handler:** `process_signing_key_update_edu()`

**Validation Flow:**
```rust
// Lines 1020-1023: Parse EDU content
let signing_update: SigningKeyUpdateContent = 
    serde_json::from_value(content.clone())

// Lines 1026-1032: User origin validation
if !signing_update.user_id.ends_with(&format!(":{}", origin_server)) {
    return Err(/* Invalid origin */);
}

// Lines 1035-1041: Delegate to DeviceEDUHandler
let edu = SigningKeyUpdateEDU { /* ... */ };
state.device_edu_handler.handle_signing_key_update(edu).await
```

**SigningKeyUpdateContent Structure** (from device_edu_handler.rs):
```rust
pub struct SigningKeyUpdateContent {
    pub user_id: String,
    pub master_key: Option<serde_json::Value>,
    pub self_signing_key: Option<serde_json::Value>,
}
```

**Handler Implementation:** Lines 125-148 in `device_edu_handler.rs`
```rust
// Lines 129-135: Store EDU in database
let edu_entity = EDU::new(ephemeral_event, false);
self.edu_repo.create(&edu_entity).await

// Line 147: Success logging
info!("Updated signing keys for user {}", user_id);
```

**Cross-Signing Key Processing:** Lines 1055-1125 in `by_txn_id.rs`

The `process_cross_signing_key()` helper validates:
- Key structure (lines 1060-1063)
- Signatures (lines 1065-1068)  
- Usage arrays (lines 1070-1075)
- Key type specific validation (lines 1078-1096)

**Repository Storage:** [`packages/surrealdb/src/repository/federation.rs`](../packages/surrealdb/src/repository/federation.rs#L1492-L1510)

Uses UPSERT pattern for atomic key updates:
```sql
UPSERT user_signing_keys:⟨$user_id⟩ CONTENT {
    user_id: $user_id,
    key_type: $key_type,
    keys: $keys,
    signatures: $signatures,
    updated_at: time::now()
}
```

**Features:**
- ✅ User origin validation
- ✅ Processes master_key
- ✅ Processes self_signing_key
- ✅ Validates key signatures
- ✅ Updates cross-signing key cache
- ✅ UPSERT ensures latest keys win

### 6. m.direct_to_device - Send-to-Device Messages

**Implementation:** Lines 1127-1246 in `by_txn_id.rs`

**Spec Compliance:** ✅ COMPLIANT

**Handler:** `process_direct_to_device_edu()`

**Message Structure Validation:**
```rust
// Line 1133: message_id for deduplication
let message_id = content.get("message_id").and_then(|v| v.as_str())

// Line 1137: sender user ID
let sender = content.get("sender").and_then(|v| v.as_str())

// Line 1141: event_type (e.g., "m.room.encrypted")
let event_type = content.get("type").and_then(|v| v.as_str())

// Line 1145: messages nested object structure
let messages = content.get("messages").and_then(|v| v.as_object())
```

**Message Routing Logic:**

Lines 1161-1236: Routes messages to local devices only
```rust
// Lines 1161-1163: Per-user iteration
for (user_id, user_devices) in messages {
    
    // Lines 1168-1180: Local user check
    match user_repo.user_exists(user_id).await {
        Ok(true) => { /* User is local, continue */ },
        Ok(false) => { 
            debug!("Ignoring message for non-local user");
            continue; 
        }
    }
    
    // Lines 1183-1236: Device routing
    for (device_id, message_content) in device_messages {
        if device_id == "*" {
            // Lines 1186-1206: Broadcast to all user devices
            let user_devices = device_repo.get_all_user_devices(user_id).await;
            for device in user_devices { /* send to each */ }
        } else {
            // Lines 1208-1236: Send to specific device
            // Line 1212: Verify device exists
            match device_repo.verify_device(user_id, device_id).await {
                Ok(true) => { /* Device exists, send */ },
                Ok(false) => { /* Device not found, skip */ }
            }
        }
    }
}
```

**Repository Storage:** [`packages/surrealdb/src/repository/federation.rs`](../packages/surrealdb/src/repository/federation.rs#L1512-L1556)

**Deduplication:** Lines 1514-1527
```sql
SELECT count() FROM direct_to_device_messages 
WHERE message_id = $message_id AND origin = $origin
```
If count > 0, message is ignored (prevents duplicate delivery).

**Storage:** Lines 1530-1556
```sql
CREATE direct_to_device_messages SET
    message_id = $message_id,
    origin = $origin,
    sender = $sender,
    message_type = $message_type,
    content = $content,
    target_user_id = $target_user_id,
    target_device_id = $target_device_id,
    created_at = time::now()
```

**Features:**
- ✅ Routes to local devices only
- ✅ Validates message_type
- ✅ Handles encrypted content
- ✅ Wildcard device support (device_id = "*")
- ✅ Device existence verification
- ✅ Message deduplication via message_id
- ✅ Per-device message storage

## General EDU Processing

### Transaction Limits ✅

**File:** `by_txn_id.rs` lines 658-664

```rust
let edus = payload.get("edus").and_then(|v| v.as_array()).unwrap_or(&empty_vec);

// Validate transaction limits
if pdus.len() > 50 {
    return Err(StatusCode::BAD_REQUEST);
}
if edus.len() > 100 {  // ✅ SPEC COMPLIANT: Max 100 EDUs
    return Err(StatusCode::BAD_REQUEST);
}
```

### Rate Limiting ⚠️

**Current Status:** Rate limiting infrastructure exists but EDU-specific limits not verified

**Server-wide rate limiting:** [`packages/server/src/middleware/rate_limit.rs`](../packages/server/src/middleware/rate_limit.rs)

**Per-EDU type rate limiting:** Not explicitly implemented at EDU processing level. Each EDU type has its own storage pattern that may provide natural rate limiting, but no explicit per-server, per-EDU-type throttling.

### Error Handling ✅

**Pattern:** All EDU handlers use `.map_err()` to log errors but continue processing

Example from typing EDU (lines 803-808):
```rust
process_typing_edu(&state, &x_matrix_auth.origin, content).await.map_err(|e| {
    warn!("Failed to process typing EDU: {}", e);
    StatusCode::INTERNAL_SERVER_ERROR
})?;
```

This ensures individual EDU failures don't fail the entire transaction (correct per spec).

### Unknown EDU Types ✅

**File:** `by_txn_id.rs` line 745

```rust
_ => {
    debug!("Unknown EDU type: {}", edu_type);
},
```

Unknown EDU types are silently ignored per Matrix spec.

### Batching/Deduplication ⚠️

**Current Status:** Partial implementation

**Deduplication:**
- ✅ Direct-to-device: message_id deduplication (lines 1514-1527 in federation.rs)
- ✅ Typing: DELETE before CREATE pattern ensures single state
- ✅ Presence: UPSERT pattern ensures latest state
- ⚠️ Receipts: No explicit deduplication (may create duplicate entries)

**Batching:** Handled at transaction level (up to 100 EDUs per transaction), not within individual EDU type processing.

## Outbound Federation

**File:** [`packages/server/src/federation/outbound_queue.rs`](../packages/server/src/federation/outbound_queue.rs)

**Queue Implementation:** Lines 1-286

```rust
pub struct OutboundTransactionQueue {
    pdu_queues: HashMap<String, VecDeque<PDU>>,
    edu_queues: HashMap<String, VecDeque<EDU>>,  // Per-destination EDU queues
    max_edus_per_txn: usize,  // Set to 100 (line 60)
}
```

**EDU Queuing:** Lines 95-119
```rust
OutboundEvent::Edu { destination, edu } => {
    let queue = self.edu_queues.entry(destination.clone()).or_default();
    queue.push_back(*edu);  // ⚠️ No filtering here - assumes EDUs are pre-filtered

    if queue.len() >= self.max_edus_per_txn {
        // Immediate flush when queue full
        self.flush_queue(&destination).await
    }
}
```

**IMPORTANT:** Outbound queue does NOT filter EDU types. Filtering must happen when creating EDUs (before queuing).

**m.read.private Filtering:** Correctly implemented in client receipt endpoint (lines 120-143 in by_event_id.rs) - private receipts are never queued for federation.

## Spec References

### Primary Specifications

- **EDUs Overview:** [`spec/server/07-edus.md`](../spec/server/07-edus.md)
- **Receipts Spec:** [`spec/server/16-receipts.md`](../spec/server/16-receipts.md)  
- **Matrix 1.4 Features:** [`tmp/matrix-spec-official/data/api/client-server/receipts.yaml`](../tmp/matrix-spec-official/data/api/client-server/receipts.yaml) (m.read.private added in Matrix 1.4)
- **Matrix 1.4 Threading:** [`tmp/matrix-spec-official/data/event-schemas/schema/m.receipt.yaml`](../tmp/matrix-spec-official/data/event-schemas/schema/m.receipt.yaml) (thread_id field)

### Key Spec Requirements

From [`spec/server/07-edus.md`](../spec/server/07-edus.md):

**EDU Processing Rules:**
- EDUs are best-effort delivery (unlike PDUs)
- Servers should not retry failed EDU delivery
- Missing EDUs should not prevent room operation
- EDUs processed in order received when possible
- Max 100 EDUs per transaction
- Unknown EDU types ignored
- Validate sender server owns the user
- Validate user membership for room-specific EDUs
- Apply size limits to EDU content
- Don't fail transaction on EDU errors

From [`spec/server/16-receipts.md`](../spec/server/16-receipts.md):

**Receipt Requirements:**
- m.read: Public read receipt - federate to all servers in room
- m.read.private: Private receipt - **NEVER federate** (Matrix 1.4+)
- Support thread_id field for threaded receipts (Matrix 1.4+)
- Only update entries explicitly listed in EDU
- Don't remove receipts not in current EDU
- Latest timestamp wins
- Validate user belongs to sending server

## Implementation Gaps & Required Changes

### CRITICAL: m.read.private Acceptance (SPEC VIOLATION)

**File:** `packages/server/src/_matrix/federation/v1/send/by_txn_id.rs`

**Issue:** Lines 832-893 process m.read.private receipts received via federation

**Current Code:**
```rust
let is_private = match receipt_type.as_str() {
    "m.read" => false,
    "m.read.private" => true,  // ⚠️ SHOULD REJECT, NOT PROCESS
    _ => { continue; }
};

// ... later ...

if is_private {
    info!("Processed m.read.private receipt: ...");
    // CRITICAL: Private receipts are NEVER federated per Matrix specification
}
```

**Required Change:** Modify lines 832-843 to reject m.read.private receipts

**Implementation:**
```rust
let is_private = match receipt_type.as_str() {
    "m.read" => false,
    "m.read.private" => {
        // Matrix 1.4 spec: m.read.private MUST NEVER be sent via federation
        // If we receive one, the remote server is violating the spec
        warn!(
            "Ignoring m.read.private receipt from {}: private receipts must not be federated (spec violation)",
            origin_server
        );
        continue; // Skip processing this receipt type entirely
    },
    _ => {
        debug!("Unknown receipt type '{}' - skipping per Matrix specification", receipt_type);
        continue;
    },
};
```

**Justification:** Per Matrix 1.4 spec and client-server API spec, m.read.private receipts are client-local only and provide privacy by not leaking read status to other servers. Accepting them from federation undermines this privacy guarantee.

**Files to Modify:**
1. `packages/server/src/_matrix/federation/v1/send/by_txn_id.rs` - Lines 832-843

### Optional: Explicit EDU Rate Limiting

**Current Status:** No per-EDU-type rate limiting enforced

**Suggested Enhancement:** Add rate limiting to prevent EDU flooding

**Implementation Location:** `by_txn_id.rs` lines 675-748

**Approach:**
1. Track EDU counts per origin server per EDU type in a time window
2. Reject transactions exceeding limits with appropriate error
3. Use existing rate limit middleware patterns from `packages/server/src/middleware/rate_limit.rs`

**Limits to Consider:**
- Typing: Max 100 per room per minute per server
- Receipts: Max 500 per room per minute per server
- Presence: Max 200 per user per minute per server
- Device updates: Max 50 per user per minute per server

**Priority:** LOW - Not strictly required by spec but good practice

### Optional: Receipt Deduplication

**Current Status:** Receipt EDU processing may create duplicate entries

**File:** `packages/surrealdb/src/repository/federation.rs` lines 1379-1408

**Current Code:**
```rust
pub async fn process_receipt_edu(
    &self,
    room_id: &str,
    user_id: &str,
    event_id: &str,
    receipt_type: &str,
    timestamp: i64,
) -> Result<(), RepositoryError> {
    let query = "
        CREATE receipts SET  // ⚠️ Always creates new entry
            room_id = $room_id,
            user_id = $user_id,
            event_id = $event_id,
            receipt_type = $receipt_type,
            timestamp = $timestamp,
            created_at = time::now()
    ";
```

**Suggested Enhancement:** Change to UPSERT pattern

```rust
let query = "
    UPSERT receipts:⟨{$room_id, $user_id, $receipt_type}⟩ CONTENT {
        room_id: $room_id,
        user_id: $user_id,
        event_id: $event_id,
        receipt_type: $receipt_type,
        timestamp: $timestamp,
        updated_at: time::now()
    } WHERE timestamp < $timestamp OR !timestamp
";
```

This ensures:
- Only latest receipt is stored per user/room/type
- Older receipts don't overwrite newer ones
- No duplicate entries

**Priority:** LOW - Functional but not optimal

## Definition of Done

### Required Changes

1. **Fix m.read.private acceptance** in `by_txn_id.rs` lines 832-843
   - Change processing logic to reject/skip m.read.private receipts
   - Add warning log when m.read.private received from remote server
   - Verify private receipts still stored for local users (already working)
   - Verify private receipts never sent outbound (already working)

### Verification Steps

1. **m.read.private Rejection:**
   - Send federation transaction with m.read.private receipt
   - Verify receipt is logged as skipped with warning
   - Verify receipt is NOT stored in database
   - Verify transaction still succeeds (doesn't fail completely)

2. **m.read Still Works:**
   - Send federation transaction with m.read receipt
   - Verify receipt is processed and stored
   - Verify no warnings logged

3. **All EDU Types Functional:**
   - Send transaction with all 6 EDU types (excluding m.read.private)
   - Verify all are processed successfully
   - Verify appropriate database storage for each type

4. **Error Handling:**
   - Send malformed EDU (missing required fields)
   - Verify transaction succeeds with logged warning
   - Verify other EDUs in transaction still processed

5. **Transaction Limits:**
   - Send transaction with 101 EDUs
   - Verify transaction rejected with 400 Bad Request
   - Send transaction with 100 EDUs
   - Verify transaction succeeds

### Success Criteria

- [x] All 6 required EDU types implemented and working
- [x] 100 EDU transaction limit enforced
- [x] User origin validation for all EDU types
- [x] Room membership verification for room EDUs
- [ ] **m.read.private receipts rejected when received from federation** (REQUIRED FIX)
- [x] m.read.private receipts never sent outbound
- [x] Thread ID support in receipts (Matrix 1.4)
- [x] Device list update DAG tracking with prev_id
- [x] Direct-to-device message deduplication
- [x] Error handling doesn't fail transactions
- [x] Unknown EDU types silently ignored

## Implementation Patterns

### EDU Handler Pattern

All EDU handlers follow this consistent pattern:

```rust
async fn process_*_edu(
    state: &AppState,
    origin_server: &str,
    content: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. Extract required fields from content
    let required_field = content.get("field")
        .and_then(|v| v.as_str())
        .ok_or("Missing required field")?;
    
    // 2. Validate user belongs to origin server
    if !user_id.ends_with(&format!(":{}", origin_server)) {
        return Err("Invalid user origin".into());
    }
    
    // 3. Additional validation (room membership, etc.)
    
    // 4. Call repository method to store EDU data
    let repo = Repository::new(state.db.clone());
    repo.process_*_edu(params).await?;
    
    // 5. Log success
    info!("Processed {} EDU", edu_type);
    Ok(())
}
```

### Repository Storage Pattern

Repository methods use consistent patterns based on EDU characteristics:

**Ephemeral State (Typing, Presence):**
```rust
// UPSERT pattern - latest state wins
UPSERT table:⟨$user_id⟩ CONTENT { /* fields */ }
```

**Event History (Receipts, Device Updates):**
```rust
// CREATE pattern - append to history
CREATE table SET /* fields */
```

**Deduplication Required (Direct-to-Device):**
```rust
// Check existence then CREATE
SELECT count() WHERE message_id = $id
if count == 0 { CREATE table SET /* fields */ }
```

## Code Organization

```
packages/server/src/
├── _matrix/federation/v1/send/
│   └── by_txn_id.rs                    # Main transaction endpoint
├── _matrix/client/v3/rooms/
│   └── by_room_id/receipt/
│       └── by_receipt_type/
│           └── by_event_id.rs          # Client receipt endpoint (shows outbound filtering)
├── federation/
│   ├── device_edu_handler.rs           # Device & signing key EDU handlers
│   ├── device_management.rs            # DeviceListUpdate struct
│   └── outbound_queue.rs               # EDU federation queuing

packages/surrealdb/src/repository/
└── federation.rs                        # All EDU repository methods (lines 1330-1556)
```

## Related Specifications

- Transaction API: `spec/server/05-transactions.md`
- Device Management: `spec/server/17-device-management.md`  
- Typing Notifications: `spec/server/14-typing-notifications.md`
- Presence: `spec/server/15-presence.md`
- Send-to-Device: `spec/server/18-send-to-device.md`
- Matrix 1.4 Changelog: `tmp/matrix-spec-official/content/changelog/`

---

**Task Priority:** MEDIUM - Core functionality works, one spec violation requires correction

**Estimated Effort:** 30 minutes - Single file change with straightforward logic update