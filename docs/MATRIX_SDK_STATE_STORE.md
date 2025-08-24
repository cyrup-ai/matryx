# Matrix SDK StateStore Trait Documentation - v0.10.0

## Overview

The StateStore trait is an abstract interface for implementing persistent storage backends in the Matrix Rust SDK. It defines methods for storing and retrieving various Matrix-related data, including room state, messages, and user information.

## Sources

- [Matrix SDK Base Documentation - StateStore Trait](https://docs.rs/matrix-sdk-base/0.10.0/matrix_sdk_base/store/trait.StateStore.html)
- [Matrix Rust SDK GitHub Repository](https://github.com/matrix-org/matrix-rust-sdk/blob/main/crates/matrix-sdk-base/src/store/traits.rs)

## Trait Definition

```rust
#[async_trait]
pub trait StateStore: Send + Sync + 'static {
    /// The error type this state store can return.
    type Error: Error + Into<StoreError> + Send + Sync + 'static;

    /// Get the current sync token that should be used for the next sync.
    async fn get_sync_token(&self) -> Result<Option<String>, Self::Error>;

    /// Set the sync token that should be used for the next sync.
    async fn set_sync_token(&self, token: &str) -> Result<(), Self::Error>;

    /// Get the filter id that should be used for the next sync.
    async fn get_filter_id(&self) -> Result<Option<String>, Self::Error>;

    /// Set the filter id that should be used for the next sync.
    async fn set_filter_id(&self, filter_id: &str) -> Result<(), Self::Error>;

    /// Get a user profile for the given room id and user id.
    async fn get_profile(
        &self,
        room_id: &RoomId,
        user_id: &UserId,
    ) -> Result<Option<MinimalRoomMemberEvent>, Self::Error>;

    /// Save a set of changes to the store.
    ///
    /// This can include changes to rooms, state events, account data, and
    /// presence events.
    async fn save_changes(&self, changes: StateChanges) -> Result<(), Self::Error>;

    /// Get a state event out of the store.
    async fn get_state_event(
        &self,
        room_id: &RoomId,
        event_type: &str,
        state_key: &str,
    ) -> Result<Option<Raw<AnyRoomState>>, Self::Error>;

    /// Get all the state events with the given type for the given room.
    ///
    /// The events should be returned with their associated state key.
    async fn get_state_events(
        &self,
        room_id: &RoomId,
        event_type: &str,
    ) -> Result<Vec<(String, Raw<AnyRoomState>)>, Self::Error>;

    /// Get all the state events of the current state of the room.
    async fn get_state_events_for_room(
        &self,
        room_id: &RoomId,
    ) -> Result<Vec<Raw<AnyRoomState>>, Self::Error>;

    /// Get the current presence event for the given user.
    async fn get_presence_event(
        &self,
        user_id: &UserId,
    ) -> Result<Option<Raw<PresenceEvent>>, Self::Error>;

    /// Get the account data event for the given type.
    async fn get_account_data_event(
        &self,
        event_type: &str,
    ) -> Result<Option<Raw<AnyGlobalAccountDataEvent>>, Self::Error>;

    /// Get the room account data event for the given room and event type.
    async fn get_room_account_data_event(
        &self,
        room_id: &RoomId,
        event_type: &str,
    ) -> Result<Option<Raw<AnyRoomAccountDataEvent>>, Self::Error>;

    /// Get a custom value, as a string, from the store.
    async fn get_custom_value(&self, key: &str) -> Result<Option<String>, Self::Error>;

    /// Get a custom value, as a byte array, from the store.
    async fn get_custom_value_raw(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error>;

    /// Set custom value, as a string, in the store.
    async fn set_custom_value(&self, key: &str, value: String) -> Result<(), Self::Error>;

    /// Set custom value, as a byte array, in the store.
    async fn set_custom_value_raw(&self, key: &str, value: Vec<u8>) -> Result<(), Self::Error>;

    /// Remove a custom value from the store.
    async fn remove_custom_value(&self, key: &str) -> Result<(), Self::Error>;

    /// Remove a room from the store.
    ///
    /// This should delete all the data that is associated with the given room.
    async fn remove_room(&self, room_id: &RoomId) -> Result<(), Self::Error>;

    /// Get a request that should be sent out of the send queue, the request
    /// will be removed from the queue.
    ///
    /// If no request can be found `None` will be returned.
    async fn get_send_queue_request(
        &self,
        queue_id: &str,
    ) -> Result<Option<QueuedRequest>, Self::Error>;

    /// Put a request in the send queue.
    async fn save_send_queue_request(
        &self,
        queue_id: &str,
        request: &QueuedRequest,
    ) -> Result<(), Self::Error>;

    /// Remove a request from the send queue.
    async fn remove_send_queue_request(&self, queue_id: &str) -> Result<(), Self::Error>;

    /// Get the set of requests that depend on the request with the given
    /// queue_id.
    async fn get_dependent_requests(&self, queue_id: &str) -> Result<Vec<String>, Self::Error>;

    /// Add a dependency between a to-be-sent request and a request that depends
    /// on it.
    ///
    /// The `queue_id` is the request that needs to be sent before the
    /// `dependent_id`.
    async fn add_dependent_request(
        &self,
        queue_id: &str,
        dependent_id: &str,
    ) -> Result<(), Self::Error>;

    /// Remove a dependency between a to-be-sent request and a request that
    /// depends on it.
    ///
    /// The `queue_id` is the request that needs to be sent before the
    /// `dependent_id`.
    async fn remove_dependent_request(
        &self,
        queue_id: &str,
        dependent_id: &str,
    ) -> Result<(), Self::Error>;

    /// Mark that a media upload has been started.
    async fn mark_media_upload_started(&self, request_id: &str) -> Result<(), Self::Error>;

    /// Get the list of requests that have started a media upload.
    async fn get_media_uploads(&self) -> Result<Vec<String>, Self::Error>;

    /// Mark that a media upload has been completed.
    async fn remove_media_upload(&self, request_id: &str) -> Result<(), Self::Error>;
}
```

## Implementation Notes

1. The trait is generic over an error type that can be converted into a `StoreError`
2. Uses `async_trait` to support asynchronous methods
3. Core capabilities include:
   - Storing sync tokens and filter IDs
   - Managing room state events
   - Handling user profiles and presence
   - Managing send queues and media uploads
   - Custom key-value storage

## Available Implementations

- **MemoryStore**: An in-memory implementation suitable for testing or ephemeral clients
- **SqliteStateStore**: A SQLite-based implementation providing persistent storage

## Usage Example

```rust
use matrix_sdk_base::store::{StateStore, StateStoreExt};
use matrix_sdk_sqlite::{SqliteStateStore, StateStoreConfig};

// Create a SQLite state store
let config = StateStoreConfig::new().passphrase("passphrase");
let state_store = SqliteStateStore::open("store_path", None, config).await?;

// Use the store
let sync_token = state_store.get_sync_token().await?;

// Save state changes
state_store.save_changes(changes).await?;

// Get a state event
let state_event = state_store.get_state_event(room_id, "m.room.name", "").await?;
```