//! Room wrapper with synchronous interfaces that hide async complexity
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Room
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use futures::stream::Stream;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::runtime::Handle;

use matrix_sdk::{
    room::Room as MatrixRoom,
    ruma::{
        events::{
            reaction::ReactionEventContent,
            room::message::{OriginalSyncRoomMessageEvent, RoomMessageEventContent},
            AnyStateEvent,
            AnySyncTimelineEvent,
        },
        EventId,
        OwnedEventId,
        RoomId,
        UInt,
        UserId,
    },
    RoomMemberships,
};

use crate::error::RoomError;
use futures_util::StreamExt; // Import StreamExt for receiver into_stream
use crate::error::Result as CyrumResult; // Use crate's Result type
use crate::future::{MatrixFuture, MatrixStream};
use crate::member::CyrumRoomMember;

/// Domain-specific future for room members list operations.
pub struct RoomMembersFuture(MatrixFuture<Vec<CyrumRoomMember>>);

impl RoomMembersFuture {
    pub fn new(future: MatrixFuture<Vec<CyrumRoomMember>>) -> Self {
        Self(future)
    }
}

impl Future for RoomMembersFuture {
    type Output = Result<Vec<CyrumRoomMember>, RoomError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

/// Domain-specific future for message sending operations.
pub struct MessageFuture(MatrixFuture<OwnedEventId>);

impl MessageFuture {
    pub fn new(future: MatrixFuture<OwnedEventId>) -> Self {
        Self(future)
    }
}

impl Future for MessageFuture {
    type Output = Result<OwnedEventId, RoomError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

/// Domain-specific future for room action operations.
pub struct RoomActionFuture(MatrixFuture<()>);

impl RoomActionFuture {
    pub fn new(future: MatrixFuture<()>) -> Self {
        Self(future)
    }
}

impl Future for RoomActionFuture {
    type Output = Result<(), RoomError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

/// Domain-specific future for timeline operations.
pub struct TimelineFuture(MatrixFuture<Vec<AnySyncTimelineEvent>>);

impl TimelineFuture {
    pub fn new(future: MatrixFuture<Vec<AnySyncTimelineEvent>>) -> Self {
        Self(future)
    }
}

impl Future for TimelineFuture {
    type Output = Result<Vec<AnySyncTimelineEvent>, RoomError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

/// Domain-specific stream for messages.
pub struct MessageStream(MatrixStream<OriginalSyncRoomMessageEvent>);

impl MessageStream {
    pub fn new(stream: MatrixStream<OriginalSyncRoomMessageEvent>) -> Self {
        Self(stream)
    }
}

impl Stream for MessageStream {
    type Item = Result<OriginalSyncRoomMessageEvent, RoomError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.0).poll_next(cx)
    }
}

/// A synchronous wrapper around the Matrix SDK Room.
///
/// This wrapper enables using the Room with a synchronous interface,
/// hiding all async complexity behind MatrixFuture objects that properly
/// implement the Future trait.
pub struct CyrumRoom {
    inner: Arc<MatrixRoom>,
    runtime_handle: Handle,
}

impl CyrumRoom {
    /// Create a new CyrumRoom wrapping the provided Matrix Room.
    pub fn new(inner: MatrixRoom) -> Self {
        Self {
            inner: Arc::new(inner),
            runtime_handle: Handle::current(),
        }
    }

    /// Get the inner Matrix room.
    pub fn inner(&self) -> &MatrixRoom {
        &self.inner
    }

    /// Get the room ID.
    pub fn room_id(&self) -> &RoomId {
        self.inner.room_id()
    }

    /// Get the room name.
    pub fn name(&self) -> Option<String> {
        self.inner.name().map(|name| name.to_string())
    }

    /// Get the room topic.
    pub fn topic(&self) -> Option<String> {
        self.inner.topic().map(|topic| topic.to_string())
    }

    /// Get the room avatar URL.
    pub fn avatar_url(&self) -> Option<String> {
        self.inner.avatar_url().map(|url| url.to_string())
    }

    /// Get the list of room members.
    pub fn members(&self) -> RoomMembersFuture {
        let room = self.inner.clone(); // Clone the Arc<Room>

        RoomMembersFuture::new(MatrixFuture::spawn(async move {
            // Call members directly on the Room
            let members =
                room.members(RoomMemberships::JOIN).await.map_err(RoomError::matrix_sdk)?;

            Ok(members.into_iter().map(CyrumRoomMember::new).collect())
        }))
    }

    /// Get the list of room members with specific membership states.
    pub fn members_with_membership(
        &self,
        joined: bool,
        invited: bool,
        left: bool,
    ) -> RoomMembersFuture {
        let room = self.inner.clone(); // Clone the Arc<Room>

        RoomMembersFuture::new(MatrixFuture::spawn(async move {
            // Build membership filter based on parameters
            let mut memberships = RoomMemberships::empty();

            if joined {
                memberships = memberships.union(RoomMemberships::JOIN);
            }

            if invited {
                memberships = memberships.union(RoomMemberships::INVITE);
            }

            if left {
                memberships = memberships.union(RoomMemberships::LEAVE);
            }

            // Call members directly on the Room
            let members = room.members(memberships).await.map_err(RoomError::matrix_sdk)?;

            Ok(members.into_iter().map(CyrumRoomMember::new).collect())
        }))
    }

    /// Send a text message to the room.
    pub fn send_text_message(&self, message: &str, thread_id: Option<&EventId>) -> MessageFuture {
        let message = message.to_owned();
        let thread_id = thread_id.map(|id| id.to_owned()); // Use map + to_owned
        let room = self.inner.clone(); // Clone the Arc<Room>

        MessageFuture::new(MatrixFuture::spawn(async move {
            // Use the send method with TextMessageEventContent
            let content = matrix_sdk::ruma::events::room::message::TextMessageEventContent::plain(message);
            let response = if let Some(tid) = thread_id {
                 room.send(content).with_thread_id(&tid).await // Use builder pattern for thread
            } else {
                 room.send(content).await
            }.map_err(RoomError::matrix_sdk)?;

            Ok(response.event_id)
        }))
    }

    /// Send a markdown message to the room.
    pub fn send_markdown_message(
        &self,
        markdown: &str,
        thread_id: Option<&EventId>,
    ) -> MessageFuture {
        let markdown = markdown.to_owned();
        let thread_id = thread_id.map(|id| id.to_owned()); // Use map + to_owned
        let room = self.inner.clone(); // Clone the Arc<Room>

        MessageFuture::new(MatrixFuture::spawn(async move {
            // Use the correct constructor for markdown content
            let content = matrix_sdk::ruma::events::room::message::TextMessageEventContent::markdown(markdown);
            let response = if let Some(tid) = thread_id {
                 room.send(content).with_thread_id(&tid).await // Use builder pattern for thread
            } else {
                 room.send(content).await
            }.map_err(RoomError::matrix_sdk)?;

            Ok(response.event_id)
        }))
    }

    /// Send a reaction to an event.
    pub fn send_reaction(&self, event_id: &EventId, key: &str) -> MatrixFuture<OwnedEventId> {
        let event_id = event_id.to_owned();
        let key = key.to_owned();
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            // ReactionEventContent constructor might have changed
            // Check ruma docs for ReactionEventContent::new or similar
            let relation = matrix_sdk::ruma::events::reaction::ReactionEventContent::new(
                 matrix_sdk::ruma::events::relation::Annotation::new(event_id, key)
            );
            // Send the reaction relation directly
            let response = room.send_relation(relation).await.map_err(RoomError::matrix_sdk)?;

            Ok(response.event_id)
        })
    }

    /// Send a file to the room.
    pub fn send_file(
        &self,
        data: Vec<u8>,
        data: Vec<u8>,
        filename: &str,
        mime_type: &str,
    ) -> MatrixFuture<OwnedEventId> {
        let filename = filename.to_owned();
        let mime_type = mime_type.parse().map_err(|_| RoomError::InvalidParameter("Invalid mime type".into()))?; // Parse mime type
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            // Use the send_attachment helper
            let response = room.send_attachment(&filename, &mime_type, data, Default::default()) // Use Default::default() for config
                .await
                .map_err(RoomError::matrix_sdk)?;

            Ok(response.event_id)
        })
    }

    /// Redact (delete) an event.
    pub fn redact_event(
        &self,
        event_id: &EventId,
        reason: Option<&str>,
    ) -> MatrixFuture<OwnedEventId> {
        let event_id = event_id.to_owned();
        let reason = reason.map(|s| s.to_owned());
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            // Call redact directly on the Room
            let response = room
                .redact(&event_id, reason.as_deref(), None) // Check if None for txn_id is still correct
                .await
                .map_err(RoomError::matrix_sdk)?;

            Ok(response.event_id)
        })
    }

    /// Mark a message as read.
    pub fn mark_as_read(&self, event_id: &EventId) -> MatrixFuture<()> {
        let event_id = event_id.to_owned();
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            // Use set_read_receipt
            room
                .set_read_receipt(&event_id)
                .await
                .map_err(RoomError::matrix_sdk)
        })
    }

    /// Leave the room.
    pub fn leave(&self) -> MatrixFuture<()> {
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move { room.leave().await.map_err(RoomError::matrix_sdk) })
    }

    /// Invite a user to the room.
    pub fn invite_user(&self, user_id: &UserId) -> MatrixFuture<()> {
        let user_id = user_id.to_owned();
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            room.invite_user_by_id(&user_id).await.map_err(RoomError::matrix_sdk)
        })
    }

    /// Kick a user from the room.
    pub fn kick_user(&self, user_id: &UserId, reason: Option<&str>) -> MatrixFuture<()> {
        let user_id = user_id.to_owned();
        let reason = reason.map(|s| s.to_owned());
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            room
                .kick_user(&user_id, reason.as_deref())
                .await
                .map_err(RoomError::matrix_sdk)
        })
    }

    /// Ban a user from the room.
    pub fn ban_user(&self, user_id: &UserId, reason: Option<&str>) -> MatrixFuture<()> {
        let user_id = user_id.to_owned();
        let reason = reason.map(|s| s.to_owned());
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            room
                .ban_user(&user_id, reason.as_deref())
                .await
                .map_err(RoomError::matrix_sdk)
        })
    }

    /// Unban a user from the room.
    pub fn unban_user(&self, user_id: &UserId, reason: Option<&str>) -> MatrixFuture<()> {
        let user_id = user_id.to_owned();
        let reason = reason.map(|s| s.to_owned());
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            room
                .unban_user(&user_id, reason.as_deref())
                .await
                .map_err(RoomError::matrix_sdk)
        })
    }

    /// Set the room name.
    pub fn set_name(&self, name: &str) -> MatrixFuture<()> {
        let name = name.to_owned();
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            room.set_name(Some(name.as_str())).await.map_err(RoomError::matrix_sdk) // Pass Some<&str>
        })
    }

    /// Set the room topic.
    pub fn set_topic(&self, topic: &str) -> MatrixFuture<()> {
        let topic = topic.to_owned();
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            room.set_topic(Some(topic.as_str())).await.map_err(RoomError::matrix_sdk) // Pass Some<&str>
        })
    }

    /// Set whether this room is a direct message room.
    pub fn set_is_direct(&self, is_direct: bool) -> MatrixFuture<()> {
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            room.set_is_direct(is_direct).await.map_err(RoomError::matrix_sdk)
        })
    }

    /// Get the room's timeline events.
    pub fn timeline(&self, limit: u32) -> TimelineFuture {
        let room = self.inner.clone(); // Clone the Arc<Room>

        TimelineFuture::new(MatrixFuture::spawn(async move {
            // Convert u32 to UInt (ruma's unsigned integer type)
            let limit = UInt::try_from(limit)
                .map_err(|_| RoomError::InvalidParameter("Invalid limit value".into()))?;

            // Use the messages API builder pattern
            let timeline = room.messages() // Returns builder
                .limit(limit) // Set limit on builder
                .await // Await the builder
                .map_err(RoomError::matrix_sdk)?;

            // Convert to the expected return type
            let events = timeline.chunk;

            Ok(events)
        }))
    }

    /// Enable encryption in the room.
    pub fn enable_encryption(&self) -> MatrixFuture<()> {
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            room.enable_encryption().await.map_err(RoomError::matrix_sdk)
        })
    }

    /// Get a state event from the room.
    pub fn get_state_event(
        &self,
        event_type: &str,
        state_key: &str,
    ) -> MatrixFuture<Option<Raw<AnyStateEvent>>> { // Return Raw event
        let event_type = matrix_sdk::ruma::events::StateEventType::try_from(event_type) // Convert to StateEventType
            .map_err(|_| RoomError::InvalidParameter("Invalid state event type".into()))?;
        let state_key = state_key.to_owned();
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            room
                .get_state_event(event_type, &state_key) // Pass StateEventType
                .await
                .map_err(RoomError::matrix_sdk)
        })
    }

    /// Send a typing notification.
    pub fn typing_notice(&self, typing: bool) -> MatrixFuture<()> {
        let room = self.inner.clone(); // Clone the Arc<Room>

        MatrixFuture::spawn(async move {
            room.typing_notice(typing).await.map_err(RoomError::matrix_sdk)
        })
    }

    /// Subscribe to new messages in this room.
    pub fn subscribe_to_messages(&self) -> MessageStream {
        let room = self.inner.clone(); // Clone the Arc<Room>
        let room_id = room.room_id().to_owned();

        MessageStream::new(MatrixStream::spawn(async move {
            let (tx, rx) = tokio::sync::mpsc::channel(100);

            // Register event handler on the client associated with the room
            let client = room.client();
            client.add_event_handler(move |ev: OriginalSyncRoomMessageEvent, current_room: MatrixRoom| {
                // Check if the event is for the target room
                if current_room.room_id() != &room_id {
                    return async { /* Different room, ignore */ };
                }

                let tx = tx.clone();
                async move {
                    // Map potential SDK errors to our RoomError
                    let result: CyrumResult<OriginalSyncRoomMessageEvent, RoomError> = Ok(ev);
                    let _ = tx.send(result).await;
                }
            });

            // Convert receiver to stream
            Ok(tokio_stream::wrappers::ReceiverStream::new(rx))
        }))
    }
}
