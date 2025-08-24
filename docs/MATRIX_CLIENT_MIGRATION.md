# Matrix Client Migration Plan

## Overview

This document outlines the plan for migrating from direct `matrix-sdk` usage to our new `MatrixClient` wrapper which provides a cleaner, more idiomatic Rust API without using `async_trait` or returning `Box<dyn Future>`.

## Benefits

1. **Cleaner Code**: Eliminates async_trait and Box<dyn Future> from client interfaces
2. **Consistent API**: Provides a consistent API across all client interfaces
3. **Error Handling**: Improved error handling with proper error types
4. **Testability**: Easier to mock and test
5. **Performance**: Potentially better performance by avoiding unnecessary boxing of futures

## Implementation Strategy

### Phase 1: Initialize MatrixClient in Worker

1. Update the worker initialization to create and store a MatrixClient:

```rust
// In worker.rs
pub struct Worker {
    // Replace this:
    client: Client,
    
    // With this:
    client: MatrixClient,
    
    // Other fields remain the same
    // ...
}

impl Worker {
    pub fn new() -> Self {
        // Create the SurrealStateStore
        let state_store = SurrealStateStore::new(config.db_path)
            .expect("Failed to create state store");
        let maxtryx_store = MatrixStateStore::new(state_store);
        
        // Create the MatrixClient
        let client = MatrixClient::with_config(
            config.homeserver_url, 
            maxtryx_store,
            encryption_settings,
            request_config
        ).expect("Failed to create Matrix client");
        
        // Rest of initialization remains similar
        // ...
    }
}
```

### Phase 2: Update Auth Flow

1. Migrate login and registration methods:

```rust
// Replace this:
pub async fn login(&self, username: &str, password: &str) -> Result<(), Error> {
    self.client.login_username(username, password).await?;
    // ...
}

// With this:
pub fn login(&self, username: &str, password: &str) -> MatrixFuture<Result<(), Error>> {
    let result = self.client.login(username, password);
    
    // Map errors or handle additional logic
    MatrixFuture::spawn(async move {
        result.await?;
        // Additional logic
        Ok(())
    })
}
```

### Phase 3: Update Room Operations

1. Migrate room operations like sending messages, fetching history, etc:

```rust
// Replace this:
pub async fn send_message(&self, room_id: &RoomId, message: &str) -> Result<(), Error> {
    let room = self.client.get_room(room_id)
        .ok_or(Error::RoomNotFound(room_id.to_string()))?;
    
    room.send_text_message(message, None).await?;
    // ...
}

// With this:
pub fn send_message(&self, room_id: &RoomId, message: &str) -> MatrixFuture<Result<(), Error>> {
    let result = self.client.send_text_message(room_id, message);
    
    MatrixFuture::spawn(async move {
        result.await?;
        // Additional logic
        Ok(())
    })
}
```

### Phase 4: Update Event Handling

1. Migrate the event handling system to use MatrixStream:

```rust
// Replace this:
self.client.add_event_handler(move |ev: SyncStateEvent<RoomMemberEventContent>, room: Room| {
    // Handle event
});

// With this:
let mut membership_stream = self.client.subscribe_to_room_memberships();

// Process in background
tokio::spawn(async move {
    while let Some(Ok((room_id, event))) = membership_stream.next().await {
        // Handle event
    }
});
```

### Phase 5: Update Windows and UI

1. Update room windows to use the new client API:

```rust
// In chat.rs and other room windows
impl MatrixWindow for ChatWindow {
    fn send_message(&mut self, content: &str) -> ActionResult {
        let room_id = self.room_id().to_owned();
        let content = content.to_owned();
        
        // Use the MatrixClient via the store
        let future = store.worker.client.send_text_message(&room_id, &content);
        
        // The future can be awaited or we can handle it asynchronously
        // depending on how we want the UI to respond
        
        Ok(None)
    }
}
```

## Expected Challenges

1. **Handling Events**: We'll need to ensure events are properly processed and dispatched to UI components
2. **Maintaining State**: Keeping state consistent across asynchronous operations
3. **Error Handling**: Making sure errors are properly propagated and displayed
4. **Testing**: Ensuring all functionality works as expected with the new API

## Timeline

1. Phase 1: Create and initialize the MatrixClient - 1 day
2. Phase 2: Update authentication flow - 1 day
3. Phase 3: Update room operations - 2 days
4. Phase 4: Update event handling - 2 days
5. Phase 5: Update UI components - 2 days
6. Testing and refinement - 2 days

Total estimated time: 10 days
