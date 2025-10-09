# SPEC_FEDERATION_12: Verify PDU Validation Pipeline Compliance

## Status
EXISTS - Needs spec compliance verification

## Description
PDU validation exists but must follow the exact 6-step validation process from the spec.

## Spec Requirements (spec/server/06-pdus.md)

### Required 6-Step Validation Process

1. **Valid Event Check** (Changed in v1.16)
   - Event complies with room version format
   - room_id present if required by version

2. **Signature Checks**
   - All signatures valid
   - Drop event if fails

3. **Hash Checks**
   - Content hash matches
   - Redact if fails (don't drop)

4. **Auth Rules (Auth Events)**
   - Check against event's auth_events
   - Reject if fails

5. **Auth Rules (State Before)**
   - Check against state before event
   - Reject if fails

6. **Auth Rules (Current State)**
   - Check against current room state
   - Soft-fail if fails (don't reject)

### Rejection vs Soft Failure

**Rejected Events:**
- Not relayed to clients
- Not included as prev_event
- Not in current state
- Still stored for reference

**Soft-Failed Events:**
- Stored normally
- Can be prev_event
- Not in current state
- Not sent to clients
- Can be retrieved via API

## Current Implementation
**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/pdu_validator.rs`

Functions to verify:
- validate_pdu()
- check_signatures()
- check_hashes()
- check_auth_events()
- check_state_auth()
- check_current_state_auth()

## What Needs Verification

### Step 1: Valid Event
- [ ] Room version format checked
- [ ] room_id validation per version
- [ ] Event dropped if invalid

### Step 2: Signatures
- [ ] All signatures verified
- [ ] Origin server signature
- [ ] Event dropped if fails

### Step 3: Hashes
- [ ] Content hash verified
- [ ] Event redacted if fails
- [ ] NOT dropped, continues processing

### Step 4: Auth Events
- [ ] Checked against auth_events
- [ ] Rejected if fails
- [ ] Proper error reported

### Step 5: State Before
- [ ] Checked against state before event
- [ ] Rejected if fails
- [ ] State calculated correctly

### Step 6: Current State
- [ ] Checked against current state
- [ ] Soft-failed if fails
- [ ] Event still stored
- [ ] Not sent to clients

### Auth Event Selection
- [ ] m.room.create included (per version)
- [ ] m.room.power_levels if present
- [ ] Sender's m.room.member if present
- [ ] For membership events:
  - [ ] Target's m.room.member
  - [ ] m.room.join_rules for join/invite/knock
  - [ ] m.room.third_party_invite if applicable
  - [ ] Authorizing user's member event for restricted

### Rejection Handling
- [ ] Rejected events not relayed
- [ ] Not used as prev_events
- [ ] Not in current state
- [ ] Still stored in DB
- [ ] Subsequent events allowed

### Soft Failure Handling
- [ ] Event stored normally
- [ ] Can be used as prev_event
- [ ] Not in current state
- [ ] Not sent to clients
- [ ] State resolution works correctly

## Validation Result Types
```rust
enum ValidationResult {
    Valid(Event),
    Rejected { event_id: String, reason: String },
    SoftFailed { event: Event, reason: String }
}
```

## Files to Review
- `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/pdu_validator.rs`
- `/Volumes/samsung_t9/maxtryx/packages/server/src/federation/authorization.rs`
- Room version implementations

## Verification Checklist
- [ ] All 6 steps implemented in order
- [ ] Step 1: Event format validation
- [ ] Step 2: Signature verification (drop)
- [ ] Step 3: Hash verification (redact)
- [ ] Step 4: Auth events check (reject)
- [ ] Step 5: State before check (reject)
- [ ] Step 6: Current state check (soft-fail)
- [ ] Rejection behavior correct
- [ ] Soft-failure behavior correct
- [ ] Auth event selection correct
- [ ] Works with all room versions

## Priority
CRITICAL - Core federation security and correctness
