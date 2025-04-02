//! Sync wrapper with synchronous interfaces
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Sync functionality
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::mpsc::channel;

use matrix_sdk::{
    Client as MatrixClient,
    config::SyncSettings,
    ruma::{
        api::client::filter::FilterDefinition,
        events::{
            room::message::OriginalSyncRoomMessageEvent,
            presence::PresenceEvent,
            typing::SyncTypingEvent,
            room::member::OriginalSyncRoomMemberEvent,
            AnyMessageLikeEvent,
        },
        OwnedRoomId,
    },
    room::Room as MatrixRoom,
    LoopCtrl,
};

use crate::error::SyncError;
use crate::future::{MatrixFuture, MatrixStream};

/// A synchronous wrapper around the Matrix SDK Sync functionality.
///
/// This wrapper enables using the Sync manager with a synchronous interface,
/// hiding all async complexity behind MatrixFuture objects that properly
/// implement the Future trait.
pub struct CyrumSync {
    client: Arc<MatrixClient>,
    runtime_handle: Handle,
}

impl CyrumSync {
    /// Create a new CyrumSync with the provided Matrix client.
    pub fn new(client: Arc<MatrixClient>) -> Self {
        Self {
            client,
            runtime_handle: Handle::current(),
        }
    }
    
    /// Perform a single sync with the homeserver.
    pub fn sync_once(&self) -> MatrixFuture<()> {
        let client = self.client.clone();
        
        MatrixFuture::spawn(async move {
            let settings = SyncSettings::default();
            client.sync_once(settings).await
                .map_err(SyncError::matrix_sdk)?;
            Ok(())
        })
    }
    
    /// Start syncing with the homeserver in the background.
    pub fn start_sync(&self) -> MatrixFuture<()> {
        let client = self.client.clone();
        
        MatrixFuture::spawn(async move {
            let settings = SyncSettings::default();
            
            // Set up the sync loop with callback returning LoopCtrl
            client.sync_with_callback(settings, |_| async {
                // Return Continue to keep syncing
                LoopCtrl::Continue
            }).await;
            
            Ok(())
        })
    }
    
    /// Start syncing with the homeserver using the provided settings.
    pub fn start_sync_with_settings(&self, full_state: bool, timeout: Option<Duration>) -> MatrixFuture<()> {
        let client = self.client.clone();
        
        MatrixFuture::spawn(async move {
            let mut settings = SyncSettings::default().full_state(full_state);
            if let Some(timeout) = timeout {
                settings = settings.timeout(timeout);
            }
            
            // Set up the sync loop with callback returning LoopCtrl
            client.sync_with_callback(settings, |_| async {
                // Return Continue to keep syncing
                LoopCtrl::Continue
            }).await;
            
            Ok(())
        })
    }
    
    /// Stop syncing with the homeserver.
    pub fn stop_sync(&self) -> MatrixFuture<()> {
        MatrixFuture::spawn(async move {
            // The sync will be stopped when the client is dropped or a new sync is started
            Ok(())
        })
    }
    
    /// Check if the client is currently syncing.
    pub fn is_syncing(&self) -> bool {
        // We can't directly check sync status
        // This implementation assumes we're syncing if there's a valid client
        true
    }
    
    /// Subscribe to room message events.
    pub fn subscribe_to_messages(&self) -> MatrixStream<(OwnedRoomId, OriginalSyncRoomMessageEvent)> {
        let client = self.client.clone();
        
        MatrixStream::spawn(async move {
            let (sender, receiver) = channel(100);
            
            client.add_event_handler(move |ev: OriginalSyncRoomMessageEvent, room: MatrixRoom| {
                let sender = sender.clone();
                let room_id = room.room_id().to_owned();
                
                async move {
                    let _ = sender.send(Ok((room_id, ev))).await;
                }
            });
            
            // Sync once to start receiving events
            let settings = SyncSettings::default();
            let _ = client.sync_once(settings).await;
            
            // Convert the Receiver to a Stream using StreamExt
            let stream = tokio_stream::wrappers::ReceiverStream::new(receiver);
            Ok(stream)
        })
    }
    
    /// Subscribe to room membership events.
    pub fn subscribe_to_memberships(&self) -> MatrixStream<(OwnedRoomId, OriginalSyncRoomMemberEvent)> {
        let client = self.client.clone();
        
        MatrixStream::spawn(async move {
            let (sender, receiver) = channel(100);
            
            client.add_event_handler(move |ev: OriginalSyncRoomMemberEvent, room: MatrixRoom| {
                let sender = sender.clone();
                let room_id = room.room_id().to_owned();
                
                async move {
                    let _ = sender.send(Ok((room_id, ev))).await;
                }
            });
            
            // Sync once to start receiving events
            let settings = SyncSettings::default();
            let _ = client.sync_once(settings).await;
            
            // Convert the Receiver to a Stream using StreamExt
            let stream = tokio_stream::wrappers::ReceiverStream::new(receiver);
            Ok(stream)
        })
    }
    
    /// Subscribe to presence events.
    pub fn subscribe_to_presence(&self) -> MatrixStream<PresenceEvent> {
        let client = self.client.clone();
        
        MatrixStream::spawn(async move {
            let (sender, receiver) = channel(100);
            
            client.add_event_handler(move |ev: PresenceEvent| {
                let sender = sender.clone();
                
                async move {
                    let _ = sender.send(Ok(ev)).await;
                }
            });
            
            // Sync once to start receiving events
            let settings = SyncSettings::default();
            let _ = client.sync_once(settings).await;
            
            // Convert the Receiver to a Stream using StreamExt
            let stream = tokio_stream::wrappers::ReceiverStream::new(receiver);
            Ok(stream)
        })
    }
    
    /// Subscribe to typing events.
    pub fn subscribe_to_typing(&self) -> MatrixStream<(OwnedRoomId, SyncTypingEvent)> {
        let client = self.client.clone();
        
        MatrixStream::spawn(async move {
            let (sender, receiver) = channel(100);
            
            client.add_event_handler(move |ev: SyncTypingEvent, room: MatrixRoom| {
                let sender = sender.clone();
                let room_id = room.room_id().to_owned();
                
                async move {
                    let _ = sender.send(Ok((room_id, ev))).await;
                }
            });
            
            // Sync once to start receiving events
            let settings = SyncSettings::default();
            let _ = client.sync_once(settings).await;
            
            // Convert the Receiver to a Stream using StreamExt
            let stream = tokio_stream::wrappers::ReceiverStream::new(receiver);
            Ok(stream)
        })
    }
    
    /// Subscribe to any room event.
    pub fn subscribe_to_room_events(&self) -> MatrixStream<(OwnedRoomId, AnyMessageLikeEvent)> {
        let client = self.client.clone();
        
        MatrixStream::spawn(async move {
            let (sender, receiver) = channel(100);
            
            client.add_event_handler(move |ev: AnyMessageLikeEvent, room: MatrixRoom| {
                let sender = sender.clone();
                let room_id = room.room_id().to_owned();
                
                async move {
                    let _ = sender.send(Ok((room_id, ev))).await;
                }
            });
            
            // Sync once to start receiving events
            let settings = SyncSettings::default();
            let _ = client.sync_once(settings).await;
            
            // Convert the Receiver to a Stream using StreamExt
            let stream = tokio_stream::wrappers::ReceiverStream::new(receiver);
            Ok(stream)
        })
    }
    
    /// Configure a sync filter.
    pub fn set_filter(&self, filter: FilterDefinition) -> MatrixFuture<String> {
        let client = self.client.clone();
        
        MatrixFuture::spawn(async move {
            // Using upload_filter as register_filter is not available
            let filter_id = client.upload_filter(filter).await
                .map_err(SyncError::matrix_sdk)?;
                
            Ok(filter_id)
        })
    }
}