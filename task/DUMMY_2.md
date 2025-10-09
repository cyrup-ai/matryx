# DUMMY_2: Fix Dummy Channel in AppState Initialization - TEST FILES INCOMPLETE

## STATUS: PARTIALLY COMPLETE (6/10)

### ✅ COMPLETED (Production Code)
- AppState::new() signature accepts outbound_tx parameter
- AppState::new() does not create dummy channel
- AppState::with_lazy_loading_optimization() signature accepts outbound_tx parameter  
- AppState::with_lazy_loading_optimization() does not create dummy channel
- main.rs passes real outbound_tx to AppState::new()
- main.rs does not replace channel after construction

### ❌ OUTSTANDING ISSUE: Test Files Not Updated

**SEVERITY: HIGH** - Code does not compile for tests

## BACKGROUND & ARCHITECTURE

### Why This Change Exists

MaxTryX implements Matrix federation using an **outbound transaction queue** pattern for server-to-server communication. The queue batches PDUs (Persistent Data Units) and EDUs (Ephemeral Data Units) before sending them to remote homeservers, ensuring proper ordering and retry logic.

**Architecture Flow:**
```
AppState
  └─> outbound_tx (channel sender)
        │
        └─> OutboundTransactionQueue (background task)
              └─> FederationClient (HTTP requests)
```

Previously, AppState created a "dummy" channel internally that was never used. The fix requires passing the real channel from main.rs so the queue can actually receive events.

### Key Files

- **[../packages/server/src/state.rs](../packages/server/src/state.rs)** - AppState definition and constructors
- **[../packages/server/src/federation/outbound_queue.rs](../packages/server/src/federation/outbound_queue.rs)** - Queue implementation and OutboundEvent type
- **[../packages/server/src/main.rs](../packages/server/src/main.rs)** - Production initialization (CORRECT pattern)

## TYPE DEFINITIONS

### OutboundEvent Enum

Defined in [federation/outbound_queue.rs:13-17](../packages/server/src/federation/outbound_queue.rs):

```rust
/// Event to send to another homeserver
#[derive(Debug, Clone)]
pub enum OutboundEvent {
    Pdu { destination: String, pdu: Box<PDU> },
    Edu { destination: String, edu: Box<EDU> },
}
```

### Channel Type

```rust
use tokio::sync::mpsc;

// Type: Unbounded MPSC channel sender
mpsc::UnboundedSender<OutboundEvent>
```

### AppState::new() Signature

From [state.rs:90-98](../packages/server/src/state.rs):

```rust
pub fn new(
    db: Surreal<Any>,
    session_service: Arc<MatrixSessionService<Any>>,
    homeserver_name: String,
    config: &'static ServerConfig,
    http_client: Arc<reqwest::Client>,
    event_signer: Arc<EventSigner>,
    dns_resolver: Arc<MatrixDnsResolver>,
    outbound_tx: mpsc::UnboundedSender<OutboundEvent>,  // 8th parameter
) -> Result<Self, Box<dyn std::error::Error + Send + Sync>>
```

## WORKING EXAMPLE FROM PRODUCTION CODE

The **correct pattern** is implemented in [main.rs:266-275](../packages/server/src/main.rs):

```rust
// Create outbound transaction queue channel
let (outbound_tx, outbound_rx) = tokio::sync::mpsc::unbounded_channel();

// Create application state with the real outbound channel
let app_state_instance = AppState::new(
    db,
    session_service,
    homeserver_name.clone(),
    config,
    http_client.clone(),
    event_signer.clone(),
    dns_resolver.clone(),
    outbound_tx,  // 8th parameter - sender half of channel
)?;

// Spawn background task with receiver half
let queue = crate::federation::outbound_queue::OutboundTransactionQueue::new(
    outbound_rx,  // receiver half
    federation_client,
    homeserver_name.clone(),
);
tokio::spawn(async move {
    queue.run().await;
});
```

**Key Points:**
1. Create channel BEFORE AppState::new()
2. Pass `outbound_tx` (sender) as 8th parameter
3. Keep `outbound_rx` (receiver) for background queue task
4. In tests, we create the channel but don't need to spawn the queue task

## OUTSTANDING ISSUE: Test Files Not Updated

Four test call sites still call `AppState::new()` with only 7 parameters instead of the required 8:

### 1. tests/common/mod.rs:95

**File:** [../packages/server/tests/common/mod.rs](../packages/server/tests/common/mod.rs)

**Current Code (BROKEN):**
```rust
let state = AppState::new(
    db,
    session_service,
    config.homeserver_name.clone(),
    config_static,
    http_client,
    event_signer,
    dns_resolver,
)?;  // MISSING 8th parameter
```

**Context:** Inside `create_test_app()` function around line 95.

### 2. tests/common/mod.rs:181

**File:** [../packages/server/tests/common/mod.rs](../packages/server/tests/common/mod.rs)

**Current Code (BROKEN):**
```rust
let state = AppState::new(
    db,
    session_service,
    static_config.homeserver_name.clone(),
    static_config,
    http_client,
    event_signer,
    dns_resolver,
)?;  // MISSING 8th parameter
```

**Context:** Inside `create_test_app_with_db()` function around line 181.

### 3. tests/common/integration/mod.rs:107

**File:** [../packages/server/tests/common/integration/mod.rs](../packages/server/tests/common/integration/mod.rs)

**Current Code (BROKEN):**
```rust
let app_state = AppState::new(
    db_any,
    session_service,
    config.homeserver_name.clone(),
    config_static,
    http_client,
    event_signer,
    dns_resolver,
)?;  // MISSING 8th parameter
```

**Context:** Inside `MatrixTestServer::new()` method around line 107.

### 4. tests/common/integration/mod.rs:311

**File:** [../packages/server/tests/common/integration/mod.rs](../packages/server/tests/common/integration/mod.rs)

**Current Code (BROKEN):**
```rust
let app_state = AppState::new(
    db_any,
    session_service,
    config.homeserver_name.clone(),
    config_static,
    http_client,
    event_signer,
    dns_resolver,
)?;  // MISSING 8th parameter
```

**Context:** Inside `create_test_app()` function around line 311.

## REQUIRED FIXES

### Step-by-Step Fix Pattern (Apply to All 4 Locations)

For each of the 4 locations above:

**1. Add import at top of file (if not already present):**

```rust
use tokio::sync::mpsc;
use matryx_server::federation::outbound_queue::OutboundEvent;
```

**2. Create channel BEFORE AppState::new() call:**

```rust
// Create outbound channel for federation queue (tests don't spawn background task)
let (outbound_tx, _outbound_rx) = tokio::sync::mpsc::unbounded_channel();
```

**Note:** Use `_outbound_rx` with underscore prefix since tests don't spawn the queue task.

**3. Add outbound_tx as 8th parameter:**

```rust
let state = AppState::new(
    db,
    session_service,
    config.homeserver_name.clone(),
    config_static,
    http_client,
    event_signer,
    dns_resolver,
    outbound_tx,  // ADD THIS LINE
)?;
```

### Complete Fixed Example

Here's what the fixed code should look like:

```rust
// Imports at top of file
use tokio::sync::mpsc;
use matryx_server::federation::outbound_queue::OutboundEvent;

// Inside function, before AppState::new()
let (outbound_tx, _outbound_rx) = tokio::sync::mpsc::unbounded_channel();

let state = AppState::new(
    db,
    session_service,
    config.homeserver_name.clone(),
    config_static,
    http_client,
    event_signer,
    dns_resolver,
    outbound_tx,  // 8th parameter
)?;
```

## FILES TO MODIFY

### File 1: packages/server/tests/common/mod.rs

**Changes Required:** 2 locations (lines ~95 and ~181)

**Pattern:** Add channel creation and 8th parameter to both `AppState::new()` calls

### File 2: packages/server/tests/common/integration/mod.rs  

**Changes Required:** 2 locations (lines ~107 and ~311)

**Pattern:** Add channel creation and 8th parameter to both `AppState::new()` calls

## IMPORT REQUIREMENTS

Ensure these imports are present at the top of each modified file:

```rust
use tokio::sync::mpsc;
use matryx_server::federation::outbound_queue::OutboundEvent;
```

**Note:** Most test files likely already have `tokio::sync::mpsc` imported. Verify and add `OutboundEvent` import if missing.

## DEFINITION OF DONE

### Compilation Success

- [ ] All 4 test file locations updated with outbound_tx parameter
- [ ] Code compiles without errors: `cargo test -p matryx_server --no-run`

### Verification Checklist

- [ ] packages/server/tests/common/mod.rs line ~95 - Fixed
- [ ] packages/server/tests/common/mod.rs line ~181 - Fixed  
- [ ] packages/server/tests/common/integration/mod.rs line ~107 - Fixed
- [ ] packages/server/tests/common/integration/mod.rs line ~311 - Fixed

### Acceptance Criteria

1. **Compiles:** `cargo test -p matryx_server --no-run` succeeds
2. **No Dummy Channel:** All AppState instances use real outbound_tx from caller
3. **Consistent Pattern:** All test files follow the same pattern as main.rs

## NOTES

### Why Tests Don't Spawn Background Task

The outbound queue background task is NOT spawned in tests because:
1. Tests don't need real federation sending
2. The receiver `_outbound_rx` is intentionally unused (prefixed with `_`)
3. Tests focus on API behavior, not federation transmission

### Original Constraint Conflict

The original task stated "NO TESTS: Do not write or modify test code", but changing a public API signature (AppState::new) **requires** updating all call sites including tests. The constraint was impossible to satisfy.

**Resolution:** Test files MUST be updated to compile successfully.