# SPEC_FEDERATION_12: PDU Validation Pipeline Spec Compliance

## Status
EXISTS - **CRITICAL COMPLIANCE ISSUES IDENTIFIED** - Requires fixes to match Matrix specification

## Overview

The PDU (Persistent Data Unit) validation pipeline exists at `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/pdu_validator.rs` but has critical compliance issues that must be fixed to match the Matrix Server-Server API specification.

## Matrix Specification Requirements

According to [spec/server/06-pdus.md](../spec/server/06-pdus.md), whenever a server receives an event from a remote server, it MUST perform these 6 checks **in this exact order**:

### 1. Valid Event Check (Changed in v1.16)
- Event complies with room version format
- `room_id` present if required by version
- **Action on failure:** DROP the event

### 2. Signature Checks
- All signatures valid
- Origin server signature verified
- **Action on failure:** DROP the event

### 3. Hash Checks
- Content hash matches computed hash
- **Action on failure:** REDACT the event before processing further (DO NOT DROP)

### 4. Authorization Rules (Auth Events)
- Check against event's `auth_events`
- **Action on failure:** REJECT the event

### 5. Authorization Rules (State Before)
- Check against state before the event
- **Action on failure:** REJECT the event

### 6. Authorization Rules (Current State)
- Check against current room state
- **Action on failure:** SOFT-FAIL the event

## Critical Issues in Current Implementation

### Issue #1: Incorrect Step Order (CRITICAL)

**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/pdu_validator.rs` lines ~240-280

**Problem:** The implementation checks hashes (Step 3) BEFORE signatures (Step 2):

```rust
// Current incorrect order:
// Step 1: Format validation ✓
let event = self.validate_format(pdu).await?;

// Step 2: Hash Verification (WRONG - should be signatures)
self.validate_event_hashes(&event, pdu).await?;

// Step 3a/3b: Signature verification (WRONG - should be step 2)
self.event_signing_engine.validate_event_crypto(&event, &expected_servers).await?;
self.verify_event_signatures(&event, origin_server).await?;
```

**Why this matters:** 
- Failed signatures must DROP the event entirely
- Failed hashes must REDACT but continue processing
- If we check hashes first and redact, we might process an event that should have been dropped

**Fix Required:**
Move signature verification (lines ~260-275) to execute BEFORE hash verification (lines ~250-255).

```rust
// Corrected order:
// Step 1: Format validation
let event = self.validate_format(pdu).await?;

// Step 2: Signature Verification (check BEFORE hashes)
self.event_signing_engine.validate_event_crypto(&event, &expected_servers).await
    .map_err(|e| PduValidationError::SignatureError(format!("{:?}", e)))?;
    
if let Err(e) = self.verify_event_signatures(&event, origin_server).await {
    return Ok(ValidationResult::Rejected {
        event_id: event.event_id.clone(),
        reason: format!("Signature verification failed: {}", e),
    });
}

// Step 3: Hash Verification (check AFTER signatures)
let event = match self.validate_event_hashes_with_redaction(&event, pdu).await? {
    HashValidationResult::Valid => event,
    HashValidationResult::Redacted(redacted_event) => {
        warn!("Event {} failed hash check, continuing with redacted version", event.event_id);
        redacted_event
    }
};
```

### Issue #2: Hash Failure Handling (CRITICAL)

**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/pdu_validator.rs` line ~1195 (validate_event_hashes)

**Problem:** When hash verification fails, the method returns an error which causes **rejection**, but the spec says to **redact and continue**:

```rust
// Current incorrect behavior:
async fn validate_event_hashes(&self, event: &Event, pdu: &Value) 
    -> Result<(), PduValidationError> {
    // ... validation logic ...
    if provided_hash != computed_hash {
        return Err(PduValidationError::HashMismatch { /* ... */ }); // WRONG - should redact
    }
}
```

**Spec requirement:** "Passes hash checks, otherwise it is **redacted before being processed further**"

**Fix Required:**

1. Create a new result type for hash validation:

```rust
/// Result of hash validation indicating whether event should be redacted
enum HashValidationResult {
    /// Hashes are valid, use original event
    Valid,
    /// Hashes failed, use redacted event
    Redacted(Event),
}
```

2. Modify `validate_event_hashes` to return this type and perform redaction on failure:

```rust
async fn validate_event_hashes_with_redaction(
    &self,
    event: &Event,
    pdu: &Value,
) -> Result<HashValidationResult, PduValidationError> {
    let room_version = self.get_room_version(&event.room_id).await
        .unwrap_or_else(|_| "1".to_string());
    
    // Validate content hash if present
    if let Some(hashes) = pdu.get("hashes") {
        match self.validate_content_hashes(pdu, hashes, &room_version).await {
            Ok(_) => {}, // Hashes valid
            Err(hash_error) => {
                warn!("Hash validation failed for event {}: {}", event.event_id, hash_error);
                // REDACT the event instead of rejecting
                let redacted_event = self.redact_event(event, &room_version, 
                    Some(format!("Hash validation failed: {}", hash_error)));
                return Ok(HashValidationResult::Redacted(redacted_event));
            }
        }
    }
    
    // Additional hash validations...
    
    Ok(HashValidationResult::Valid)
}
```

3. Implement the `redact_event` method:

```rust
/// Redact an event according to Matrix specification redaction rules
/// 
/// Strips event content according to room version while preserving:
/// - event_id, room_id, sender, type, state_key
/// - origin_server_ts, hashes, signatures, depth, prev_events, auth_events
/// 
/// See Matrix spec: Client-Server API "Redactions" section for field preservation rules
fn redact_event(&self, event: &Event, room_version: &str, reason: Option<String>) -> Event {
    let mut redacted = event.clone();
    
    // Strip content based on room version redaction rules
    redacted.content = match event.event_type.as_str() {
        "m.room.member" => {
            // Preserve only membership field for m.room.member
            let mut preserved = serde_json::Map::new();
            if let Some(membership) = event.content.get("membership") {
                preserved.insert("membership".to_string(), membership.clone());
            }
            EventContent::Unknown(serde_json::Value::Object(preserved))
        },
        "m.room.create" => {
            // Preserve creator field for m.room.create
            let mut preserved = serde_json::Map::new();
            if let Some(creator) = event.content.get("creator") {
                preserved.insert("creator".to_string(), creator.clone());
            }
            EventContent::Unknown(serde_json::Value::Object(preserved))
        },
        "m.room.join_rules" => {
            // Preserve join_rule field
            let mut preserved = serde_json::Map::new();
            if let Some(join_rule) = event.content.get("join_rule") {
                preserved.insert("join_rule".to_string(), join_rule.clone());
            }
            EventContent::Unknown(serde_json::Value::Object(preserved))
        },
        "m.room.power_levels" => {
            // For power_levels, preserve all fields per spec
            event.content.clone()
        },
        "m.room.history_visibility" => {
            // Preserve history_visibility field
            let mut preserved = serde_json::Map::new();
            if let Some(visibility) = event.content.get("history_visibility") {
                preserved.insert("history_visibility".to_string(), visibility.clone());
            }
            EventContent::Unknown(serde_json::Value::Object(preserved))
        },
        _ => {
            // For other events, strip all content
            EventContent::Unknown(serde_json::Value::Object(serde_json::Map::new()))
        }
    };
    
    // Add redaction information to unsigned field
    let mut unsigned = redacted.unsigned
        .and_then(|u| u.as_object().cloned())
        .unwrap_or_else(|| serde_json::Map::new());
    
    unsigned.insert("redacted_because".to_string(), serde_json::json!({
        "type": "m.room.redaction",
        "reason": reason.unwrap_or_else(|| "Hash validation failed".to_string())
    }));
    
    redacted.unsigned = Some(serde_json::Value::Object(unsigned));
    
    debug!("Redacted event {} due to hash failure", event.event_id);
    redacted
}
```

### Issue #3: Validation Result Types

**Status:** ✓ CORRECT - Already properly implemented

The `ValidationResult` enum at line ~85 correctly implements the three required outcomes:

```rust
pub enum ValidationResult {
    /// Event is valid and should be accepted
    Valid(Event),
    
    /// Event failed validation and should be rejected
    Rejected { event_id: String, reason: String },
    
    /// Event should be soft-failed (stored but not used in state resolution)
    SoftFailed { event: Event, reason: String },
}
```

### Issue #4: Soft-Fail Implementation

**Status:** ✓ CORRECT - Already properly implemented

The Step 6 validation at line ~325 (`validate_current_state`) correctly implements soft-failure:

```rust
// Step 6: Current State Validation (soft-fail check)
match self.validate_current_state(&event).await {
    Ok(_) => {
        Ok(ValidationResult::Valid(event))
    },
    Err(e) => {
        // Soft-fail: event is stored but not relayed to clients
        let mut soft_failed_event = event;
        soft_failed_event.soft_failed = Some(true);
        
        Ok(ValidationResult::SoftFailed {
            event: soft_failed_event,
            reason: format!("Current state validation failed: {}", e),
        })
    },
}
```

This correctly:
- Sets the `soft_failed` flag on the event
- Returns `SoftFailed` variant (not rejection)
- Includes reason for soft-failure

## Auth Events Selection Compliance

**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/authorization.rs` lines ~480-580

**Status:** ✓ APPEARS CORRECT - Verify implementation

The `AuthEventsSelector::select_auth_events` method implements the spec requirements:

1. ✓ m.room.create event (room version dependent)
2. ✓ Current m.room.power_levels event
3. ✓ Sender's current m.room.member event
4. ✓ For m.room.member events:
   - Target's current m.room.member event
   - m.room.join_rules for join/invite/knock
   - m.room.third_party_invite for third-party invites
   - Authorizing user's member event for restricted rooms

**Verification needed:** Ensure the implementation matches spec exactly, particularly:
- Room version handling for m.room.create inclusion
- Third-party invite token matching
- Restricted room authorization flow

## Rejection vs Soft-Failure Behavior

### Rejected Events (Steps 4-5 failures)

**Spec Requirements:**
- NOT relayed to clients
- NOT included as prev_event in new events
- NOT in current state
- Still stored for reference

**Implementation Location:** Event storage and relay logic (needs verification in event handlers)

**Files to check:**
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/*/` - Federation endpoints
- Event relay logic to ensure rejected events are not sent to clients
- State resolution to ensure rejected events not included in state

### Soft-Failed Events (Step 6 failures)

**Spec Requirements:**
- Stored normally
- CAN be used as prev_event
- NOT in current state
- NOT sent to clients
- Can be retrieved via /event API

**Implementation Location:** Same as above, needs verification

## Room Version Support

**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/pdu_validator.rs` lines ~370-570

**Status:** ✓ COMPREHENSIVE - Already implemented

The validator includes room version-specific validation:
- `validate_room_v1_v2_format()` - Basic format validation
- `validate_room_v3_format()` - State resolution v2 support
- `validate_room_v4_format()` - Hash-based event IDs
- `validate_room_v5_format()` - Integer restrictions
- `validate_room_v6_format()` - Content hash requirements
- `validate_room_v7_plus_format()` - Latest enhancements

## Implementation Changes Required

### File: `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/pdu_validator.rs`

#### Change 1: Reorder Steps 2 and 3 (Lines ~240-280)

**Current code location:** `validate_pdu()` method

**Action:** Move signature verification to execute BEFORE hash verification

**Specific changes:**
1. Move lines ~260-275 (signature checks) to execute before lines ~250-255 (hash checks)
2. Ensure deduplication check remains at the top (after Step 1)
3. Update step numbers in comments to match spec

#### Change 2: Implement Hash Redaction (Lines ~1195+)

**Current code location:** `validate_event_hashes()` method

**Actions:**
1. Create `HashValidationResult` enum (add after `ValidationResult` enum definition ~line 100)
2. Rename `validate_event_hashes()` to `validate_event_hashes_with_redaction()`
3. Change return type from `Result<(), PduValidationError>` to `Result<HashValidationResult, PduValidationError>`
4. Modify hash failure handling to call `redact_event()` instead of returning error
5. Implement `redact_event()` method with room version-specific content stripping

#### Change 3: Update validate_pdu to Handle Redaction

**Current code location:** `validate_pdu()` method around line 250

**Action:** Update Step 2 (hash validation) to handle redaction:

```rust
// Step 2: Hash Verification with redaction support
let event = match self.validate_event_hashes_with_redaction(&event, pdu).await? {
    HashValidationResult::Valid => event,
    HashValidationResult::Redacted(redacted_event) => {
        warn!("Event {} failed hash check - continuing with redacted version", 
              event.event_id);
        redacted_event
    }
};
debug!("Step 2 passed: Hash verification for event {}", event.event_id);
```

## Supporting Code References

### Event Structure
**File:** `/Volumes/samsung_t9/maxtryx/packages/entity/src/types/event.rs`

The `Event` struct already has all necessary fields:
- `soft_failed: Option<bool>` - Used for Step 6 failures ✓
- `unsigned: Option<serde_json::Value>` - Can store `redacted_because` ✓
- `rejected_reason: Option<String>` - Can track rejection reasons ✓

### Authorization Engine
**File:** `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/authorization.rs`

The `AuthorizationEngine` provides:
- `authorize_event()` - Used in Steps 4-6 ✓
- `PowerLevelValidator` - Power level checks ✓
- `AuthEventsSelector` - Auth event selection ✓
- `EventTypeValidator` - Event-specific rules ✓

### Matrix Specification Reference
**File:** `/Volumes/samsung_t9/maxtryx/spec/server/06-pdus.md`

Contains the complete specification for PDU validation including:
- 6-step validation process
- Auth events selection rules
- Rejection vs soft-failure semantics
- Room version considerations

## Definition of Done

The PDU validation pipeline is compliant when:

1. **Step Order Correct:**
   - Signature verification (Step 2) executes BEFORE hash verification (Step 3)
   - All 6 steps execute in spec-defined order
   - Step numbers in code comments match spec numbering

2. **Hash Failure Redaction:**
   - Hash validation failures result in event redaction, not rejection
   - Redacted events continue through remaining validation steps
   - Redaction preserves fields per room version rules
   - `unsigned.redacted_because` field populated with reason

3. **Validation Results Correct:**
   - Step 2 failure → `ValidationResult::Rejected` (drop event)
   - Step 3 failure → Redaction applied, processing continues
   - Steps 4-5 failure → `ValidationResult::Rejected`
   - Step 6 failure → `ValidationResult::SoftFailed` with `soft_failed: true`

4. **Event Handling Correct:**
   - Rejected events: Not relayed, not used as prev_events, stored for reference
   - Soft-failed events: Stored, can be prev_events, not in state, not sent to clients
   - Valid events: Normal processing and relay

5. **Room Version Support:**
   - All validation steps work correctly across room versions 1-10
   - Redaction rules vary by room version as per spec
   - Auth events selection adapts to room version requirements

## Priority

**CRITICAL** - Core federation security and correctness

Incorrect PDU validation can lead to:
- Security vulnerabilities (accepting malicious events)
- Federation failures (rejecting valid events)
- State divergence between servers
- Protocol non-compliance

## Related Files

- Implementation: [`/Volumes/samsung_t9/maxtryx/packages/server/src/federation/pdu_validator.rs`](../packages/server/src/federation/pdu_validator.rs)
- Authorization: [`/Volumes/samsung_t9/maxtryx/packages/server/src/federation/authorization.rs`](../packages/server/src/federation/authorization.rs)
- Event Types: [`/Volumes/samsung_t9/maxtryx/packages/entity/src/types/event.rs`](../packages/entity/src/types/event.rs)
- Specification: [`/Volumes/samsung_t9/maxtryx/spec/server/06-pdus.md`](../spec/server/06-pdus.md)
