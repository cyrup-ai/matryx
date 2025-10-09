# SPEC_FEDERATION_10: Verify EDU Processing in Transactions

## Status
NEEDS VERIFICATION - EDU handling exists but needs spec compliance check

## Description
The transaction endpoint processes EDUs but needs verification against all spec requirements.

## Spec Requirements (spec/server/07-edus.md)

### EDU Types Required
1. **m.typing** - Typing notifications
2. **m.presence** - Presence updates  
3. **m.receipt** - Read receipts (m.read, m.read.private)
4. **m.device_list_update** - Device list changes
5. **m.signing_key_update** - Cross-signing key updates
6. **m.direct_to_device** - Direct messages to devices

### Key Requirements
- Max 100 EDUs per transaction
- Validate user belongs to origin server
- Verify room membership for room EDUs
- Handle m.read.private correctly (NEVER federate)
- Support threading in receipts (thread_id field)
- Process device_list_update with prev_id DAG

## Current Implementation
**Location:** `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/send/by_txn_id.rs`

Functions to verify:
- process_typing_edu()
- process_receipt_edu()
- process_presence_edu()
- process_device_list_edu()
- process_signing_key_update_edu()
- process_direct_to_device_edu()

## What Needs Verification

### 1. Typing EDU (m.typing)
- [ ] Validates user from origin server
- [ ] Checks room membership
- [ ] Updates typing state
- [ ] TTL/expiration handling
- [ ] Deduplication

### 2. Receipt EDU (m.receipt)
- [ ] Supports m.read (public)
- [ ] Supports m.read.private (NEVER federate)
- [ ] Validates per-event receipts
- [ ] Handles thread_id field (Matrix 1.4)
- [ ] Timestamp validation

### 3. Presence EDU (m.presence)
- [ ] Validates push array
- [ ] Checks user_id from origin
- [ ] Updates presence state
- [ ] Handles status_msg
- [ ] last_active_ago processing
- [ ] currently_active flag

### 4. Device List EDU (m.device_list_update)
- [ ] Validates user from origin
- [ ] Processes stream_id
- [ ] Handles prev_id DAG
- [ ] Updates device cache
- [ ] Detects gaps and resyncs
- [ ] Deleted device handling

### 5. Signing Key Update EDU
- [ ] Processes cross-signing updates
- [ ] Validates signatures
- [ ] Updates key cache

### 6. Direct to Device EDU
- [ ] Routes to local devices
- [ ] Validates message_type
- [ ] Handles encrypted content

### 7. General EDU Processing
- [ ] Max 100 EDUs enforced
- [ ] Rate limiting applied
- [ ] Unknown EDU types ignored
- [ ] Batching/deduplication
- [ ] Error handling (don't fail transaction)

## Files to Review
- `/Volumes/samsung_t9/maxtryx/packages/server/src/_matrix/federation/v1/send/by_txn_id.rs`
- EDU handler implementations
- Repository methods for EDU storage

## Spec References
- spec/server/07-edus.md - All EDU types
- spec/server/14-typing-notifications.md
- spec/server/15-presence.md  
- spec/server/16-receipts.md
- spec/server/17-device-management.md

## Verification Checklist
- [ ] All 6 EDU types supported
- [ ] m.read.private NEVER federated
- [ ] thread_id supported in receipts
- [ ] device_list_update DAG correct
- [ ] User validation for all EDUs
- [ ] Room membership checked
- [ ] 100 EDU limit enforced
- [ ] Rate limiting implemented
- [ ] Batching/deduplication working
- [ ] Error handling proper

## Priority
MEDIUM - EDUs already work but need spec compliance verification
