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

Four test call sites still call `AppState::new()` with only 7 parameters instead of the required 8:

1. **[packages/server/tests/common/mod.rs:95](../packages/server/tests/common/mod.rs)**
   ```rust
   // Line 95 - MISSING outbound_tx parameter
   let state = AppState::new(
       db,
       session_service,
       config.homeserver_name.clone(),
       config_static,
       http_client,
       event_signer,
       dns_resolver,
   )?;
   ```

2. **[packages/server/tests/common/mod.rs:181](../packages/server/tests/common/mod.rs)**
   ```rust
   // Line 181 - MISSING outbound_tx parameter
   let state = AppState::new(
       db,
       session_service,
       static_config.homeserver_name.clone(),
       static_config,
       http_client,
       event_signer,
       dns_resolver,
   )?;
   ```

3. **[packages/server/tests/common/integration/mod.rs:107](../packages/server/tests/common/integration/mod.rs)**
   ```rust
   // Line 107 - MISSING outbound_tx parameter
   let app_state = AppState::new(
       db_any,
       session_service,
       config.homeserver_name.clone(),
       config_static,
       http_client,
       event_signer,
       dns_resolver,
   )?;
   ```

4. **[packages/server/tests/common/integration/mod.rs:311](../packages/server/tests/common/integration/mod.rs)**
   ```rust
   // Line 311 - MISSING outbound_tx parameter
   let app_state = AppState::new(
       db_any,
       session_service,
       config.homeserver_name.clone(),
       config_static,
       http_client,
       event_signer,
       dns_resolver,
   )?;
   ```

## REQUIRED FIXES

### For Each Test File Location

**Add before AppState::new() call:**
```rust
// Create outbound channel for test
let (outbound_tx, _outbound_rx) = tokio::sync::mpsc::unbounded_channel();
```

**Update AppState::new() call to include:**
```rust
let state = AppState::new(
    db,
    session_service,
    config.homeserver_name.clone(),
    config_static,
    http_client,
    event_signer,
    dns_resolver,
    outbound_tx,  // ADD THIS PARAMETER
)?;
```

## TYPE DEFINITION (for reference)

```rust
use tokio::sync::mpsc;
use crate::federation::outbound_queue::OutboundEvent;

// Channel type
mpsc::UnboundedSender<OutboundEvent>
```

## CONSTRAINT CONFLICT NOTE

The task originally stated "NO TESTS: Do not write or modify test code" but changing a public API signature (AppState::new) REQUIRES updating all call sites including tests. This creates an impossible constraint.

**Resolution:** Test files MUST be updated to satisfy "Code compiles without errors" requirement.

## DEFINITION OF DONE (Remaining)

- [ ] packages/server/tests/common/mod.rs line 95 - Add outbound_tx parameter
- [ ] packages/server/tests/common/mod.rs line 181 - Add outbound_tx parameter
- [ ] packages/server/tests/common/integration/mod.rs line 107 - Add outbound_tx parameter
- [ ] packages/server/tests/common/integration/mod.rs line 311 - Add outbound_tx parameter
- [ ] Code compiles without errors: `cargo test -p matryx_server --no-run`

## IMPORT REQUIREMENTS

Ensure test files have the necessary imports:
```rust
use tokio::sync::mpsc;
use crate::federation::outbound_queue::OutboundEvent;
```

Most test files likely already have tokio::sync::mpsc imported, but verify OutboundEvent import exists.
