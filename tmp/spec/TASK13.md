# TASK 13: Send-to-Device Messaging Foundation

## OBJECTIVE
Implement the critical send-to-device messaging system required for End-to-End Encryption, including device targeting, message queuing, and federation support.

## SUBTASKS

### SUBTASK1: Send-to-Device API Endpoints
- **What**: Implement core send-to-device API endpoints
- **Where**: `packages/server/src/_matrix/client/v3/send_to_device/` (create)
- **Why**: Enable direct device-to-device messaging required for E2EE

### SUBTASK2: Device Targeting System
- **What**: Add device targeting including wildcard (`*`) support for all devices
- **Where**: `packages/server/src/device/targeting.rs` (create)
- **Why**: Allow messages to be sent to specific devices or all user devices

### SUBTASK3: Message Queuing and Delivery
- **What**: Implement message queuing and reliable delivery system
- **Where**: `packages/server/src/device/message_queue.rs` (create)
- **Why**: Ensure messages are delivered even when devices are offline

### SUBTASK4: Federation Support
- **What**: Add federation support for cross-server device messaging
- **Where**: `packages/server/src/device/federation.rs` (create)
- **Why**: Enable send-to-device messaging across Matrix federation

### SUBTASK5: Message Persistence and Cleanup
- **What**: Implement message persistence with automatic cleanup
- **Where**: `packages/server/src/device/persistence.rs` (create)
- **Why**: Store messages reliably while managing storage efficiently

## DEFINITION OF DONE
- Send-to-device endpoints functional and tested
- Device targeting working including wildcard support
- Message queuing operational with delivery guarantees
- Federation messaging working across servers
- Message persistence with proper cleanup
- Clean compilation with `cargo fmt && cargo check`

## RESEARCH NOTES
- Matrix send-to-device specification
- Device messaging patterns for E2EE
- Message queuing reliability requirements
- Federation device messaging protocols

## REQUIRED DOCUMENTATION
- Matrix send-to-device specification
- Device messaging API documentation
- Message queuing implementation guides
- Federation device messaging specification