# Cyrup-Matrix Wrapper Library Plan

## Overview

The `cyrup-matrix` crate will provide a complete, idiomatic Rust wrapper around the matrix-sdk library, eliminating async_trait and Box<dyn Future> usage while providing a clean, domain-specific API.

## Core Components

### 1. Client API

- **CyrumClient**: Main entry point for Matrix operations
  - Authentication (login, registration, logout)
  - Room management
  - User management
  - Sync control
  - Cross-signing and verification
  - Well-defined error types

### 2. Room API

- **CyrumRoom**: Room operations wrapper
  - Message sending (text, media, reactions)
  - Message history and timeline access
  - Room state management (name, topic, permissions)
  - Member management
  - Clean API for encryption settings
  - Redaction and event management

### 3. Storage

- **CyrumStateStore**: Clean wrapper around Matrix storage
  - Room state persistence
  - Account data
  - Media cache
  - Keys and encryption data
  - Custom data storage

### 4. Encryption

- **CyrumEncryption**: E2EE wrapper with clean interfaces
  - Verification management
  - Key backup and recovery
  - Cross-signing operations
  - Security status and recommendations

### 5. Media

- **CyrumMedia**: Media operations wrapper
  - Upload with progress tracking
  - Download with cancelation
  - Thumbnail generation
  - Media encryption

### 6. Sync & Events

- **CyrumSync**: Sync management
  - Incremental sync configuration
  - Event subscription (with type-safe streams)
  - Presence updates
  - Typing notifications
  - Read receipts

### 7. Notification Settings

- **CyrumNotifications**: Notification management
  - Per-room notification settings
  - Global notification rules
  - Push rules management

## Design Principles

1. **No async_trait**: Avoid `async_trait` macro usage entirely
2. **No Box<dyn Future>**: Never return opaque future types
3. **Clean Error Handling**: Well-defined error hierarchy
4. **Type Safety**: Strong typing throughout the API
5. **Domain-Specific Types**: Use concrete types instead of generic ones
6. **Hidden Complexity**: Hide all async complexity with MatrixFuture/MatrixStream

## Implementation Strategy

1. **Incremental Development**:
   - Start with most commonly used components
   - Add more specialized functionality over time
   - Ensure backward compatibility

2. **Testing**:
   - Comprehensive unit tests for all components
   - Integration tests with mock server
   - Property-based testing where appropriate

3. **Documentation**:
   - Clear, concise API docs with examples
   - Usage guides for common patterns
   - Migration guides from raw matrix-sdk

## Implementation Order

1. **Phase 1: Core Components** (Already started)
   - CyrumStateStore (✅ Completed)
   - CyrumClient (✅ Completed)
   - Error types and result handling (✅ Completed)

2. **Phase 2: Immediate Needs**
   - CyrumRoom
   - Basic event handling
   - Authentication flow

3. **Phase 3: Enhanced Functionality**
   - CyrumEncryption
   - CyrumMedia
   - CyrumSync (advanced features)

4. **Phase 4: Specialized Features**
   - CyrumNotifications
   - CyrumSpaces
   - Advanced room features

## Example Usage

```rust
// Create a client
let store = CyrumStateStore::new(surreal_store);
let client = CyrumClient::with_config("https://matrix.org", store, None, None)?;

// Login
client.login("username", "password").await?;

// Get a room
let room = client.get_room(&room_id)?;

// Send a message and get the event ID
let event_id = room.send_text_message("Hello, world!").await?;

// Subscribe to new messages
let mut message_stream = client.subscribe_to_messages();
while let Some(Ok((room_id, event))) = message_stream.next().await {
    println!("New message in {}: {}", room_id, event.content.body());
}
```

## Benefits Over Direct matrix-sdk Usage

1. **Simplified Async**: No dealing with async/await complexity
2. **Better Error Messages**: Domain-specific errors with clear contexts
3. **Type Safety**: No need to handle raw JSON or generic events
4. **Consistency**: Uniform API patterns across all components
5. **Performance**: Potentially better performance with optimized implementations
6. **Testing**: Easier to mock and test

## Timeline

- Phase 1: Core components - Already completed
- Phase 2: Immediate needs - 1 week
- Phase 3: Enhanced functionality - 2 weeks
- Phase 4: Specialized features - 2 weeks

Total: 5 weeks for complete implementation