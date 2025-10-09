# SPEC_FEDERATION_10: EDU Processing - Final Fix Required

## Status: 9/10 - One Critical Spec Violation

**QA Rating: 9/10** - Implementation is 95% complete with production quality. All 6 required EDU types are fully implemented with proper validation, storage, and real-time propagation. However, ONE critical Matrix specification violation prevents full compliance.

## Critical Issue: m.read.private Receipt Acceptance

**Severity:** CRITICAL - Spec Violation  
**File:** `packages/server/src/_matrix/federation/v1/send/by_txn_id.rs`  
**Lines:** 832-843

### Current Behavior (INCORRECT)

The server currently accepts and processes `m.read.private` receipts received via federation:

```rust
let is_private = match receipt_type.as_str() {
    "m.read" => false,
    "m.read.private" => true,  // ❌ PROCESSES instead of REJECTING
    _ => { continue; }
};
```

Lines 874-887 then process and store the private receipt.

### Matrix Specification Requirement

Per Matrix 1.4 specification (client-server API spec, receipts.yaml):
- `m.read.private` receipts are **client-local only**
- They provide privacy by NOT leaking read status to other servers
- They **MUST NEVER be sent via federation**
- Receiving them indicates the remote server is violating the spec

**Spec References:**
- `spec/server/16-receipts.md` - Receipt federation rules
- `tmp/matrix-spec-official/data/api/client-server/receipts.yaml` - m.read.private added in Matrix 1.4

### Required Fix

**File:** `packages/server/src/_matrix/federation/v1/send/by_txn_id.rs`  
**Lines:** 832-843

Replace the current receipt type matching with:

```rust
let is_private = match receipt_type.as_str() {
    "m.read" => false,
    "m.read.private" => {
        // Matrix 1.4 spec: m.read.private MUST NEVER be sent via federation
        // If we receive one, the remote server is violating the spec
        warn!(
            "Rejecting m.read.private receipt from {}: private receipts must not be federated (spec violation by remote server)",
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

### Why This Matters

Accepting `m.read.private` receipts from federation undermines the privacy guarantee that Matrix 1.4 provides to users. Private receipts allow users to see their own read status without revealing it to other servers or users in the room.

### Verification Steps

After implementing the fix:

1. **Test m.read.private rejection:**
   - Send federation transaction with m.read.private receipt
   - Verify receipt is logged with warning about spec violation
   - Verify receipt is NOT stored in database
   - Verify transaction still succeeds (other EDUs processed)

2. **Test m.read still works:**
   - Send federation transaction with m.read receipt
   - Verify receipt is processed and stored
   - Verify no warnings logged

3. **Test local private receipts still work:**
   - Client posts m.read.private via `/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}`
   - Verify stored locally in database
   - Verify NOT sent to federation (already correct - verified at line 142 in by_event_id.rs)

### Notes

**Outbound Filtering (Already Correct):**
- File: `packages/server/src/_matrix/client/v3/rooms/by_room_id/receipt/by_receipt_type/by_event_id.rs`
- Lines: 120-142
- m.read.private receipts are correctly stored locally but NEVER federated
- No changes needed on outbound side

**All Other EDUs (Already Correct):**
- m.typing: ✅ Complete with user validation, room membership checks, 30s TTL
- m.receipt (m.read): ✅ Complete with thread_id support (Matrix 1.4)
- m.presence: ✅ Complete with UPSERT pattern for latest state
- m.device_list_update: ✅ Complete with DAG tracking (prev_id)
- m.signing_key_update: ✅ Complete with key validation
- m.direct_to_device: ✅ Complete with message_id deduplication

**Transaction Processing (Already Correct):**
- ✅ 100 EDU max limit enforced (line 503)
- ✅ Unknown EDU types silently ignored (line 670)
- ✅ Error handling doesn't fail transactions
- ✅ User origin validation for all EDU types

## Definition of Done

- [ ] Fix m.read.private rejection in by_txn_id.rs (lines 832-843)
- [ ] Verify private receipts are rejected with warning log
- [ ] Verify private receipts are NOT stored when received from federation
- [ ] Verify m.read receipts still process normally
- [ ] Verify local private receipts still work (client → server, stored locally only)

Once this single fix is implemented and verified, the implementation will be **10/10 - Fully Spec Compliant**.

---

**Estimated Effort:** 5-10 minutes  
**Priority:** HIGH - Spec compliance violation
