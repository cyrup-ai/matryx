# SPEC_FEDERATION_12: PDU Validation Pipeline Spec Compliance - CRITICAL FIXES REQUIRED

## Status
**CRITICAL COMPLIANCE VIOLATIONS** - 2 core issues must be fixed

## QA Rating: 3/10

### What Works ✓
- ValidationResult enum with Valid/Rejected/SoftFailed states
- Step 6 soft-fail implementation (correct flag setting)
- Authorization validation using AuthorizationEngine
- Room version support (v1-v10)
- DAG validation and cycle detection
- Event deduplication logic

### Critical Issues Remaining

## Issue #1: Incorrect Step Order (CRITICAL SPEC VIOLATION)

**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/pdu_validator.rs` lines 248-276

**Current Code (WRONG):**
```rust
// Step 2: Hash Verification
self.validate_event_hashes(&event, pdu).await?;
debug!("Step 2 passed: Hash verification for event {}", event.event_id);

// Step 3a: EventSigningEngine verification
self.event_signing_engine.validate_event_crypto(&event, &expected_servers).await
    .map_err(|e| PduValidationError::SignatureError(format!("{:?}", e)))?;
debug!("Step 3a passed: EventSigningEngine verification for event {}", event.event_id);

// Step 3b: Additional event signature verification
if let Err(e) = self.verify_event_signatures(&event, origin_server).await {
    warn!("Step 3b failed - event signature verification for event {}: {}", event.event_id, e);
    return Ok(ValidationResult::Rejected {
        event_id: event.event_id.clone(),
        reason: format!("Event signature verification failed: {}", e),
    });
}
```

**Required Fix:**
Signatures MUST be verified BEFORE hashes. Swap the order:

```rust
// Step 2: Signature Verification (MUST BE BEFORE HASHES)
let expected_servers = vec![origin_server.to_string()];
self.event_signing_engine.validate_event_crypto(&event, &expected_servers).await
    .map_err(|e| PduValidationError::SignatureError(format!("{:?}", e)))?;
debug!("Step 2a passed: EventSigningEngine verification for event {}", event.event_id);

if let Err(e) = self.verify_event_signatures(&event, origin_server).await {
    warn!("Step 2b failed - signature verification for event {}: {}", event.event_id, e);
    return Ok(ValidationResult::Rejected {
        event_id: event.event_id.clone(),
        reason: format!("Signature verification failed: {}", e),
    });
}
debug!("Step 2 passed: Signature verification for event {}", event.event_id);

// Step 3: Hash Verification with Redaction Support (AFTER SIGNATURES)
let event = match self.validate_event_hashes_with_redaction(&event, pdu).await? {
    HashValidationResult::Valid => event,
    HashValidationResult::Redacted(redacted_event) => {
        warn!("Event {} failed hash check - continuing with redacted version", event.event_id);
        redacted_event
    }
};
debug!("Step 3 passed: Hash verification for event {}", event.event_id);
```

**Why This Matters:**
- Signature failures → DROP the event entirely (it's cryptographically invalid)
- Hash failures → REDACT but continue processing (event may still be valid)
- Current order risks processing events that should be dropped

---

## Issue #2: Hash Failure Must Redact, Not Reject (CRITICAL SPEC VIOLATION)

**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/pdu_validator.rs` lines 1171-1280

**Current Code (WRONG):**
```rust
async fn validate_event_hashes(&self, event: &Event, pdu: &Value) 
    -> Result<(), PduValidationError> {
    // ...
    if provided_hash != computed_hash {
        return Err(PduValidationError::HashMismatch { 
            expected: computed_hash,
            actual: provided_hash.to_string(),
        }); // ❌ WRONG - This causes rejection
    }
}
```

**Matrix Spec Requirement:** 
"Passes hash checks, otherwise it is **redacted before being processed further**"

### Required Changes:

#### 1. Add HashValidationResult enum (after line 95)

```rust
/// Result of hash validation indicating whether event should be redacted
#[derive(Debug)]
enum HashValidationResult {
    /// Hashes are valid, use original event
    Valid,
    /// Hashes failed, use redacted event
    Redacted(Event),
}
```

#### 2. Rename and change validate_event_hashes method (line 1171)

**Change from:**
```rust
async fn validate_event_hashes(&self, event: &Event, pdu: &Value) 
    -> Result<(), PduValidationError>
```

**To:**
```rust
async fn validate_event_hashes_with_redaction(&self, event: &Event, pdu: &Value) 
    -> Result<HashValidationResult, PduValidationError>
```

#### 3. Modify hash failure handling in validate_content_hashes (line 1198)

**Change from:**
```rust
if provided_hash != computed_hash {
    return Err(PduValidationError::HashMismatch {
        expected: computed_hash,
        actual: provided_hash.to_string(),
    });
}
```

**To:**
```rust
match self.validate_content_hashes(pdu, hashes, &room_version).await {
    Ok(_) => {}, // Hashes valid
    Err(hash_error) => {
        warn!("Hash validation failed for event {}: {}", event.event_id, hash_error);
        // REDACT instead of rejecting
        let redacted_event = self.redact_event(event, &room_version, 
            Some(format!("Hash validation failed: {}", hash_error)));
        return Ok(HashValidationResult::Redacted(redacted_event));
    }
}

// All hashes valid
Ok(HashValidationResult::Valid)
```

#### 4. Implement redact_event method (add after line 1280)

```rust
/// Redact an event according to Matrix specification redaction rules
/// 
/// Strips event content according to room version while preserving:
/// - event_id, room_id, sender, type, state_key (for state events)
/// - origin_server_ts, hashes, signatures, depth, prev_events, auth_events
/// 
/// Content preservation varies by event type per Matrix spec redaction rules
fn redact_event(&self, event: &Event, room_version: &str, reason: Option<String>) -> Event {
    let mut redacted = event.clone();
    
    // Strip content based on Matrix redaction rules for each event type
    redacted.content = match event.event_type.as_str() {
        "m.room.member" => {
            // Preserve only membership field
            let mut preserved = serde_json::Map::new();
            if let Some(membership) = event.content.get("membership") {
                preserved.insert("membership".to_string(), membership.clone());
            }
            EventContent::Unknown(serde_json::Value::Object(preserved))
        },
        "m.room.create" => {
            // Preserve creator field
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

---

## Definition of Done

Both issues are fixed when:

1. **Step Order:**
   - Signature validation (Step 2) executes BEFORE hash validation (Step 3)
   - Code comments correctly label steps matching Matrix spec
   - `validate_pdu()` method implements correct order

2. **Hash Redaction:**
   - `HashValidationResult` enum exists and is used
   - `validate_event_hashes_with_redaction()` method implemented
   - Hash failures call `redact_event()` instead of returning error
   - `redact_event()` properly strips content per event type
   - Redacted events continue through remaining validation steps (Step 4-6)
   - `unsigned.redacted_because` field populated with reason

3. **Testing:**
   - Events with invalid signatures are REJECTED (not processed)
   - Events with invalid hashes are REDACTED and continue validation
   - Redacted events can still pass/fail Steps 4-6 normally

---

## Priority

**CRITICAL** - Core Matrix federation compliance and security

Current implementation will:
- ❌ Fail to interoperate with other Matrix homeservers
- ❌ Violate Matrix Server-Server API specification
- ❌ Reject valid events (should redact instead)
- ❌ Potentially process invalid events

## Files to Modify

- `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/pdu_validator.rs`

## Related Specification

- `/Volumes/samsung_t9/maxtryx/spec/server/06-pdus.md` - Defines 6-step validation process
