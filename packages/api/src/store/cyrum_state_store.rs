use futures::stream::Stream;
use matrix_sdk_base::store::{QueuedRequest, StateChanges};
use matrix_sdk_base::{
    deserialized_responses::{DisplayName, RawAnySyncOrStrippedState},
    ruma::{
        events::presence::PresenceEvent,
        events::receipt::{Receipt, ReceiptThread, ReceiptType},
        events::{
            AnyGlobalAccountDataEvent,
            AnyRoomAccountDataEvent,
            GlobalAccountDataEventType,
            RoomAccountDataEventType,
            StateEventType,
        },
        serde::Raw,
        EventId,
        MilliSecondsSinceUnixEpoch,
        OwnedEventId,
        OwnedRoomId,
        OwnedTransactionId,
        OwnedUserId,
        RoomId,
        TransactionId,
        UserId,
    },
    store::{
        ChildTransactionId,
        DependentQueuedRequest,
        DependentQueuedRequestKind,
        QueueWedgeError,
        QueuedRequestKind,
        SentRequestKey,
        StateStoreDataKey,
        StateStoreDataValue,
    },
    MinimalRoomMemberEvent,
    RoomInfo,
    RoomMemberships,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::error::Result;

// Domain-specific future types

/// A future for key-value data operations
pub struct KeyValueFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for state changes operations
pub struct StateChangesFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for presence event operations
pub struct PresenceFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for state event operations
pub struct StateEventFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for profile operations
pub struct ProfileFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for room info operations
pub struct RoomInfoFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for display name operations
pub struct DisplayNameFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for account data operations
pub struct AccountDataFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for receipt operations
pub struct ReceiptFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for custom value operations
pub struct CustomValueFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for room operations
pub struct RoomFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for send queue operations
pub struct SendQueueFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for dependent queue operations
pub struct DependentQueueFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

/// A future for media upload operations
pub struct MediaUploadFuture<T>(Pin<Box<dyn Future<Output = Result<T>> + Send>>);

// Domain-specific stream types

/// A stream of presence events
pub struct PresenceStream(Pin<Box<dyn Stream<Item = Result<Raw<PresenceEvent>>> + Send>>);

/// A stream of state events
pub struct StateEventStream(Pin<Box<dyn Stream<Item = Result<RawAnySyncOrStrippedState>> + Send>>);

/// A stream of state event pairs
pub struct StateEventPairStream(
    Pin<Box<dyn Stream<Item = Result<(String, RawAnySyncOrStrippedState)>> + Send>>,
);

/// A stream of user IDs
pub struct UserIdStream(Pin<Box<dyn Stream<Item = Result<OwnedUserId>> + Send>>);

/// A stream of room IDs
pub struct RoomIdStream(Pin<Box<dyn Stream<Item = Result<OwnedRoomId>> + Send>>);

/// A stream of room info objects
pub struct RoomInfoStream(Pin<Box<dyn Stream<Item = Result<RoomInfo>> + Send>>);

/// A stream of receipt pairs
pub struct ReceiptStream(Pin<Box<dyn Stream<Item = Result<(OwnedUserId, Receipt)>> + Send>>);

/// A stream of account data events
pub struct GlobalAccountDataStream(
    Pin<Box<dyn Stream<Item = Result<AnyGlobalAccountDataEvent>> + Send>>,
);

/// A stream of room account data events
pub struct RoomAccountDataStream(
    Pin<Box<dyn Stream<Item = Result<AnyRoomAccountDataEvent>> + Send>>,
);

/// A stream of queued requests
pub struct QueuedRequestStream(Pin<Box<dyn Stream<Item = Result<QueuedRequest>> + Send>>);

/// A stream of dependent queued requests
pub struct DependentQueuedRequestStream(
    Pin<Box<dyn Stream<Item = Result<DependentQueuedRequest>> + Send>>,
);

/// A stream of media upload IDs
pub struct MediaUploadStream(Pin<Box<dyn Stream<Item = Result<String>> + Send>>);

// Implement Future for all domain-specific future types

macro_rules! impl_future {
    ($future_type:ident) => {
        impl<T> Future for $future_type<T> {
            type Output = Result<T>;

            fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                // Forward to the inner future
                let inner = self.get_mut();
                Pin::new(&mut inner.0).poll(cx)
            }
        }

        impl<T> $future_type<T> {
            /// Creates a new domain-specific future from a boxed future
            pub fn new<F>(future: F) -> Self
            where
                F: Future<Output = Result<T>> + Send + 'static,
            {
                Self(Box::pin(future))
            }
        }
    };
}

// Implement Stream for all domain-specific stream types

macro_rules! impl_stream {
    ($stream_type:ident, $item_type:ty) => {
        impl Stream for $stream_type {
            type Item = Result<$item_type>;

            fn poll_next(
                mut self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                // Forward to the inner stream
                let inner = self.get_mut();
                Pin::new(&mut inner.0).poll_next(cx)
            }
        }

        impl $stream_type {
            /// Creates a new domain-specific stream
            pub fn new<S>(stream: S) -> Self
            where
                S: Stream<Item = Result<$item_type>> + Send + 'static,
            {
                Self(Box::pin(stream))
            }
        }
    };
}

// Implement Future for all domain-specific future types
impl_future!(KeyValueFuture);
impl_future!(StateChangesFuture);
impl_future!(PresenceFuture);
impl_future!(StateEventFuture);
impl_future!(ProfileFuture);
impl_future!(RoomInfoFuture);
impl_future!(DisplayNameFuture);
impl_future!(AccountDataFuture);
impl_future!(ReceiptFuture);
impl_future!(CustomValueFuture);
impl_future!(RoomFuture);
impl_future!(SendQueueFuture);
impl_future!(DependentQueueFuture);
impl_future!(MediaUploadFuture);

// Implement Stream for all domain-specific stream types
impl_stream!(PresenceStream, Raw<PresenceEvent>);
impl_stream!(StateEventStream, RawAnySyncOrStrippedState);
impl_stream!(StateEventPairStream, (String, RawAnySyncOrStrippedState));
impl_stream!(UserIdStream, OwnedUserId);
impl_stream!(RoomIdStream, OwnedRoomId);
impl_stream!(RoomInfoStream, RoomInfo);
impl_stream!(ReceiptStream, (OwnedUserId, Receipt));
impl_stream!(GlobalAccountDataStream, AnyGlobalAccountDataEvent);
impl_stream!(RoomAccountDataStream, AnyRoomAccountDataEvent);
impl_stream!(QueuedRequestStream, QueuedRequest);
impl_stream!(DependentQueuedRequestStream, DependentQueuedRequest);
impl_stream!(MediaUploadStream, String);

/// A trait defining the state storage operations for Matrix client data
/// with a synchronous interface following the "Hidden Box/Pin" pattern.
///
/// This trait provides domain-specific future types for individual operations,
/// and stream types for list operations, maintaining exact method signatures
/// while hiding the async complexity.
pub trait MatrixStateStore: Send + Sync + 'static {
    /// Get key-value data from the store.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to fetch data for.
    fn get_kv_data(
        &self,
        key: StateStoreDataKey<'_>,
    ) -> KeyValueFuture<Option<StateStoreDataValue>>;

    /// Put key-value data into the store.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to identify the data in the store.
    ///
    /// * `value` - The data to insert.
    ///
    /// Panics if the key and value variants do not match.
    fn set_kv_data(
        &self,
        key: StateStoreDataKey<'_>,
        value: StateStoreDataValue,
    ) -> KeyValueFuture<()>;

    /// Remove key-value data from the store.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to remove the data for.
    fn remove_kv_data(&self, key: StateStoreDataKey<'_>) -> KeyValueFuture<()>;

    /// Save the set of state changes in the store.
    fn save_changes(&self, changes: &StateChanges) -> StateChangesFuture<()>;

    /// Get the stored presence event for the given user.
    ///
    /// # Arguments
    ///
    /// * `user_id` - The id of the user for which we wish to fetch the presence
    /// event for.
    fn get_presence_event(&self, user_id: &UserId) -> PresenceFuture<Option<Raw<PresenceEvent>>>;

    /// Get the stored presence events for the given users.
    ///
    /// # Arguments
    ///
    /// * `user_ids` - The IDs of the users to fetch the presence events for.
    fn get_presence_events(&self, user_ids: &[OwnedUserId]) -> PresenceStream;

    /// Get a state event out of the state store.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room the state event was received for.
    ///
    /// * `event_type` - The event type of the state event.
    fn get_state_event(
        &self,
        room_id: &RoomId,
        event_type: StateEventType,
        state_key: &str,
    ) -> StateEventFuture<Option<RawAnySyncOrStrippedState>>;

    /// Get a list of state events for a given room and `StateEventType`.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room to find events for.
    ///
    /// * `event_type` - The event type.
    fn get_state_events(&self, room_id: &RoomId, event_type: StateEventType) -> StateEventStream;

    /// Get a list of state events for a given room, `StateEventType`, and the
    /// given state keys.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room to find events for.
    ///
    /// * `event_type` - The event type.
    ///
    /// * `state_keys` - The list of state keys to find.
    fn get_state_events_for_keys(
        &self,
        room_id: &RoomId,
        event_type: StateEventType,
        state_keys: &[&str],
    ) -> StateEventStream;

    /// Get the current profile for the given user in the given room.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The room id the profile is used in.
    ///
    /// * `user_id` - The id of the user the profile belongs to.
    fn get_profile(
        &self,
        room_id: &RoomId,
        user_id: &UserId,
    ) -> ProfileFuture<Option<MinimalRoomMemberEvent>>;

    /// Get the current profiles for the given users in the given room.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The ID of the room the profiles are used in.
    ///
    /// * `user_ids` - The IDs of the users the profiles belong to.
    fn get_profiles<'a>(
        &self,
        room_id: &RoomId,
        user_ids: &'a [OwnedUserId],
    ) -> ProfileFuture<BTreeMap<OwnedUserId, MinimalRoomMemberEvent>>;

    /// Get the user ids of members for a given room with the given memberships,
    /// for stripped and regular rooms alike.
    fn get_user_ids(&self, room_id: &RoomId, memberships: RoomMemberships) -> UserIdStream;

    /// Get all the pure `RoomInfo`s the store knows about.
    fn get_room_infos(&self) -> RoomInfoStream;

    /// Get all the users that use the given display name in the given room.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room for which the display name users should
    /// be fetched for.
    ///
    /// * `display_name` - The display name that the users use.
    fn get_users_with_display_name(
        &self,
        room_id: &RoomId,
        display_name: &DisplayName,
    ) -> DisplayNameFuture<BTreeSet<OwnedUserId>>;

    /// Get all the users that use the given display names in the given room.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The ID of the room to fetch the display names for.
    ///
    /// * `display_names` - The display names that the users use.
    fn get_users_with_display_names<'a>(
        &self,
        room_id: &RoomId,
        display_names: &'a [DisplayName],
    ) -> DisplayNameFuture<HashMap<DisplayName, BTreeSet<OwnedUserId>>>;

    /// Get an event out of the account data store.
    ///
    /// # Arguments
    ///
    /// * `event_type` - The event type of the account data event.
    fn get_account_data_event(
        &self,
        event_type: GlobalAccountDataEventType,
    ) -> AccountDataFuture<Option<Raw<AnyGlobalAccountDataEvent>>>;

    /// Get an event out of the room account data store.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room for which the room account data event
    ///   should
    /// be fetched.
    ///
    /// * `event_type` - The event type of the room account data event.
    fn get_room_account_data_event(
        &self,
        room_id: &RoomId,
        event_type: RoomAccountDataEventType,
    ) -> AccountDataFuture<Option<Raw<AnyRoomAccountDataEvent>>>;

    /// Get an event out of the user room receipt store.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room for which the receipt should be
    ///   fetched.
    ///
    /// * `receipt_type` - The type of the receipt.
    ///
    /// * `thread` - The thread containing this receipt.
    ///
    /// * `user_id` - The id of the user for who the receipt should be fetched.
    fn get_user_room_receipt_event(
        &self,
        room_id: &RoomId,
        receipt_type: ReceiptType,
        thread: ReceiptThread,
        user_id: &UserId,
    ) -> ReceiptFuture<Option<(OwnedEventId, Receipt)>>;

    /// Get events out of the event room receipt store.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The id of the room for which the receipts should be
    ///   fetched.
    ///
    /// * `receipt_type` - The type of the receipts.
    ///
    /// * `thread` - The thread containing this receipt.
    ///
    /// * `event_id` - The id of the event for which the receipts should be
    ///   fetched.
    fn get_event_room_receipt_events(
        &self,
        room_id: &RoomId,
        receipt_type: ReceiptType,
        thread: ReceiptThread,
        event_id: &EventId,
    ) -> ReceiptStream;

    /// Get arbitrary data from the custom store
    ///
    /// # Arguments
    ///
    /// * `key` - The key to fetch data for
    fn get_custom_value(&self, key: &[u8]) -> CustomValueFuture<Option<Vec<u8>>>;

    /// Put arbitrary data into the custom store, return the data previously
    /// stored
    ///
    /// # Arguments
    ///
    /// * `key` - The key to insert data into
    ///
    /// * `value` - The value to insert
    fn set_custom_value(&self, key: &[u8], value: Vec<u8>) -> CustomValueFuture<Option<Vec<u8>>>;

    /// Put arbitrary data into the custom store, do not attempt to read any
    /// previous data
    ///
    /// Optimization option for set_custom_values for stores that would perform
    /// better withouts the extra read and the caller not needing that data
    /// returned. Otherwise this just wraps around `set_custom_data` and
    /// discards the result.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to insert data into
    ///
    /// * `value` - The value to insert
    fn set_custom_value_no_read(&self, key: &[u8], value: Vec<u8>) -> CustomValueFuture<()> {
        let future = self.set_custom_value(key, value);
        CustomValueFuture::new(async move { future.await.map(|_| ()) })
    }

    /// Remove arbitrary data from the custom store and return it if existed
    ///
    /// # Arguments
    ///
    /// * `key` - The key to remove data from
    fn remove_custom_value(&self, key: &[u8]) -> CustomValueFuture<Option<Vec<u8>>>;

    /// Remove a room and all elements associated from the state store.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The `RoomId` of the room to delete.
    fn remove_room(&self, room_id: &RoomId) -> RoomFuture<()>;

    /// Save a request to be sent by a send queue later (e.g. sending an event).
    ///
    /// # Arguments
    ///
    /// * `room_id` - The `RoomId` of the send queue's room.
    /// * `transaction_id` - The unique key identifying the event to be sent
    ///   (and its transaction). Note: this is expected to be randomly generated
    ///   and thus unique.
    /// * `content` - Serializable event content to be sent.
    fn save_send_queue_request(
        &self,
        room_id: &RoomId,
        transaction_id: OwnedTransactionId,
        created_at: MilliSecondsSinceUnixEpoch,
        request: QueuedRequestKind,
        priority: usize,
    ) -> SendQueueFuture<()>;

    /// Updates a send queue request with the given content, and resets its
    /// error status.
    ///
    /// # Arguments
    ///
    /// * `room_id` - The `RoomId` of the send queue's room.
    /// * `transaction_id` - The unique key identifying the request to be sent
    ///   (and its transaction).
    /// * `content` - Serializable event content to replace the original one.
    ///
    /// Returns true if a request has been updated, or false otherwise.
    fn update_send_queue_request(
        &self,
        room_id: &RoomId,
        transaction_id: &TransactionId,
        content: QueuedRequestKind,
    ) -> SendQueueFuture<bool>;

    /// Remove a request previously inserted with
    /// [`Self::save_send_queue_request`] from the database, based on its
    /// transaction id.
    ///
    /// Returns true if something has been removed, or false otherwise.
    fn remove_send_queue_request(
        &self,
        room_id: &RoomId,
        transaction_id: &TransactionId,
    ) -> SendQueueFuture<bool>;

    /// Loads all the send queue requests for the given room.
    ///
    /// The resulting vector of queued requests should be ordered from higher
    /// priority to lower priority, and respect the insertion order when
    /// priorities are equal.
    fn load_send_queue_requests(&self, room_id: &RoomId) -> QueuedRequestStream;

    /// Updates the send queue error status (wedge) for a given send queue
    /// request.
    fn update_send_queue_request_status(
        &self,
        room_id: &RoomId,
        transaction_id: &TransactionId,
        error: Option<QueueWedgeError>,
    ) -> SendQueueFuture<()>;

    /// Loads all the rooms which have any pending requests in their send queue.
    fn load_rooms_with_unsent_requests(&self) -> RoomIdStream;

    /// Add a new entry to the list of dependent send queue requests for a
    /// parent request.
    fn save_dependent_queued_request(
        &self,
        room_id: &RoomId,
        parent_txn_id: &TransactionId,
        own_txn_id: ChildTransactionId,
        created_at: MilliSecondsSinceUnixEpoch,
        content: DependentQueuedRequestKind,
    ) -> DependentQueueFuture<()>;

    /// Mark a set of dependent send queue requests as ready, using a key
    /// identifying the homeserver's response.
    ///
    /// ⚠ Beware! There's no verification applied that the parent key type is
    /// compatible with the dependent event type. The invalid state may be
    /// lazily filtered out in `load_dependent_queued_requests`.
    ///
    /// Returns the number of updated requests.
    fn mark_dependent_queued_requests_as_ready(
        &self,
        room_id: &RoomId,
        parent_txn_id: &TransactionId,
        sent_parent_key: SentRequestKey,
    ) -> DependentQueueFuture<usize>;

    /// Update a dependent send queue request with the new content.
    ///
    /// Returns true if the request was found and could be updated.
    fn update_dependent_queued_request(
        &self,
        room_id: &RoomId,
        own_transaction_id: &ChildTransactionId,
        new_content: DependentQueuedRequestKind,
    ) -> DependentQueueFuture<bool>;

    /// Remove a specific dependent send queue request by id.
    ///
    /// Returns true if the dependent send queue request has been indeed
    /// removed.
    fn remove_dependent_queued_request(
        &self,
        room: &RoomId,
        own_txn_id: &ChildTransactionId,
    ) -> DependentQueueFuture<bool>;

    /// List all the dependent send queue requests.
    ///
    /// This returns absolutely all the dependent send queue requests, whether
    /// they have a parent event id or not. As a contract for implementors, they
    /// must be returned in insertion order.
    fn load_dependent_queued_requests(&self, room: &RoomId) -> DependentQueuedRequestStream;

    /// Mark a media upload as started.
    ///
    /// Tracks the media upload with the given request ID as started.
    /// This helps with resuming uploads after client restarts.
    fn mark_media_upload_started(&self, request_id: &str) -> MediaUploadFuture<()>;

    /// Get all ongoing media uploads.
    ///
    /// Returns a stream of request IDs for all media uploads that are in progress.
    /// Used when the client starts to resume unfinished uploads.
    fn get_media_uploads(&self) -> MediaUploadStream;

    /// Remove a media upload.
    ///
    /// Marks the media upload as completed or removes it from tracking.
    /// This should be called when an upload is completed or cancelled.
    fn remove_media_upload(&self, request_id: &str) -> MediaUploadFuture<()>;
}
