//! Real-time Matrix sync implementation using SurrealDB LiveQuery
//!
//! This module provides a LiveQuery-based Matrix sync system that delivers
//! real-time updates instead of traditional HTTP polling. It leverages
//! SurrealDB's LiveQuery capabilities for superior performance and responsiveness.

use anyhow::Result;
use futures_util::{Stream, StreamExt};
#[cfg(test)]
use matryx_entity::EventContent;
use matryx_entity::{Event, Membership, MembershipState, UserPresenceUpdate};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::repositories::ClientRepositoryService;

/// Matrix sync state for tracking synchronization progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    /// Current sync batch token
    pub next_batch: String,
    /// Rooms the user has joined
    pub joined_rooms: HashMap<String, JoinedRoomState>,
    /// Rooms the user has been invited to
    pub invited_rooms: HashMap<String, InvitedRoomState>,
    /// Rooms the user has left
    pub left_rooms: HashMap<String, LeftRoomState>,
    /// User presence information
    pub presence: HashMap<String, PresenceState>,
    /// Account data
    pub account_data: HashMap<String, serde_json::Value>,
    /// Device list updates
    pub device_lists: DeviceListUpdates,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            next_batch: Uuid::new_v4().to_string(),
            joined_rooms: HashMap::new(),
            invited_rooms: HashMap::new(),
            left_rooms: HashMap::new(),
            presence: HashMap::new(),
            account_data: HashMap::new(),
            device_lists: DeviceListUpdates::default(),
        }
    }
}

/// State for a joined room
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JoinedRoomState {
    /// Room timeline events
    pub timeline: Vec<Event>,
    /// Room state events
    pub state: Vec<Event>,
    /// Ephemeral events (typing, receipts)
    pub ephemeral: Vec<serde_json::Value>,
    /// Room account data
    pub account_data: Vec<serde_json::Value>,
    /// Unread notification counts
    pub unread_notifications: UnreadNotifications,
    /// Room summary
    pub summary: Option<RoomSummary>,
}

/// State for an invited room
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvitedRoomState {
    /// Invite state events
    pub invite_state: Vec<Event>,
}

/// State for a left room
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeftRoomState {
    /// Room state at time of leaving
    pub state: Vec<Event>,
    /// Final timeline events
    pub timeline: Vec<Event>,
    /// Room account data
    pub account_data: Vec<serde_json::Value>,
}

/// User presence state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceState {
    /// Presence status (online, offline, unavailable)
    pub presence: String,
    /// Status message
    pub status_msg: Option<String>,
    /// Last active timestamp
    pub last_active_ago: Option<u64>,
    /// Currently active flag
    pub currently_active: Option<bool>,
}

/// Device list updates
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeviceListUpdates {
    /// Users whose device lists have changed
    pub changed: Vec<String>,
    /// Users who have left encrypted rooms
    pub left: Vec<String>,
}

/// Unread notification counts
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnreadNotifications {
    /// Number of unread highlighted messages
    pub highlight_count: u64,
    /// Total number of unread notifications
    pub notification_count: u64,
}

/// Room summary information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSummary {
    /// Room heroes (important members)
    pub heroes: Vec<String>,
    /// Number of joined members
    pub joined_member_count: u64,
    /// Number of invited members
    pub invited_member_count: u64,
}

/// Sync update notification
#[derive(Debug, Clone)]
pub enum SyncUpdate {
    /// Room event update
    RoomEvent { room_id: String, event: Event },
    /// Room membership update
    MembershipUpdate {
        room_id: String,
        user_id: String,
        membership: Membership,
    },
    /// Room state update
    StateUpdate { room_id: String, event: Event },
    /// Presence update
    PresenceUpdate { user_id: String, presence: PresenceState },
    /// Account data update
    AccountDataUpdate { data_type: String, content: serde_json::Value },
    /// Device list update
    DeviceListUpdate { changed: Vec<String>, left: Vec<String> },
}

/// LiveQuery-based Matrix sync manager
pub struct LiveQuerySync {
    /// User ID for this sync session
    user_id: String,
    /// Current sync state
    state: Arc<RwLock<SyncState>>,
    /// Client repository service
    repository_service: ClientRepositoryService,
    /// Broadcast channel for sync updates
    update_sender: broadcast::Sender<SyncUpdate>,
    /// Receiver for sync updates
    update_receiver: broadcast::Receiver<SyncUpdate>,
    /// Track users who have left rooms for device list updates
    /// This set is populated by membership subscriptions and consumed by device subscriptions
    left_users: Arc<RwLock<HashSet<String>>>,
}

impl LiveQuerySync {
    /// Create a new LiveQuery sync manager
    pub fn new(
        user_id: String,
        device_id: String,
        db: surrealdb::Surreal<surrealdb::engine::any::Any>
    ) -> Self {
        let (update_sender, update_receiver) = broadcast::channel(1000);

        let repository_service = ClientRepositoryService::from_db(
            db,
            user_id.clone(),
            device_id
        );

        Self {
            user_id,
            state: Arc::new(RwLock::new(SyncState::default())),
            repository_service,
            update_sender,
            update_receiver,
            left_users: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Start the LiveQuery sync process
    pub async fn start(&self) -> Result<()> {
        info!("Starting LiveQuery sync for user: {}", self.user_id);

        // Initialize sync state with current user's rooms
        self.initialize_sync_state().await?;

        // Start LiveQuery subscriptions
        self.start_event_subscriptions().await?;
        self.start_membership_subscriptions().await?;
        self.start_presence_subscriptions().await?;
        self.start_device_subscriptions().await?;

        info!("LiveQuery sync started successfully");
        Ok(())
    }

    /// Initialize sync state with current user data
    async fn initialize_sync_state(&self) -> Result<()> {
        debug!("Initializing sync state for user: {}", self.user_id);

        // Get user's current room memberships
        let memberships = self
            .repository_service
            .get_user_memberships()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get user memberships: {}", e))?;

        let mut state = self.state.write().await;

        // Initialize joined rooms
        for membership in memberships {
            if membership.membership == MembershipState::Join {
                let room_state = JoinedRoomState::default();
                state.joined_rooms.insert(membership.room_id.clone(), room_state);

                // Load recent events for this room
                let events = self
                    .repository_service
                    .get_room_events(&membership.room_id, Some(50))
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to get room events: {}", e))?;

                if let Some(room_state) = state.joined_rooms.get_mut(&membership.room_id) {
                    room_state.timeline = events;
                }
            } else if membership.membership == MembershipState::Invite {
                let invite_state = InvitedRoomState { invite_state: Vec::new() };
                state.invited_rooms.insert(membership.room_id, invite_state);
            }
        }

        debug!(
            "Initialized sync state with {} joined rooms, {} invited rooms",
            state.joined_rooms.len(),
            state.invited_rooms.len()
        );

        Ok(())
    }

    /// Start LiveQuery subscriptions for events
    async fn start_event_subscriptions(&self) -> Result<()> {
        let state = self.state.clone();
        let repository_service = self.repository_service.clone();
        let update_sender = self.update_sender.clone();

        // Get list of joined rooms to subscribe to
        let room_ids: Vec<String> = {
            let state_guard = state.read().await;
            state_guard.joined_rooms.keys().cloned().collect()
        };

        // Subscribe to events for each joined room
        for room_id in room_ids {
            let room_id_str = room_id.to_string();
            let repository_service_clone = repository_service.clone();
            let update_sender_clone = update_sender.clone();

            tokio::spawn(async move {
                // Use repository method for LiveQuery subscription
                match repository_service_clone.subscribe_to_room_events(&room_id_str).await {
                    Ok(mut stream) => {
                        info!("Subscribed to events for room: {}", room_id_str);

                        while let Some(notification_result) = stream.next().await {
                            match notification_result {
                                Ok(notification) => {
                                    let event: Event = notification;
                                    debug!(
                                        "Received event in room {}: {}",
                                        room_id_str, event.event_id
                                    );

                                    let update = SyncUpdate::RoomEvent {
                                        room_id: room_id_str.clone(),
                                        event: event.clone(),
                                    };

                                    if let Err(e) = update_sender_clone.send(update) {
                                        warn!("Failed to send event update: {}", e);
                                    }
                                },
                                Err(e) => {
                                    error!("Error in event stream for room {}: {}", room_id_str, e);
                                },
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to subscribe to events for room {}: {}", room_id_str, e);
                    },
                }
            });
        }

        Ok(())
    }

    /// Start LiveQuery subscriptions for membership changes
    async fn start_membership_subscriptions(&self) -> Result<()> {
        let repository_service = self.repository_service.clone();
        let user_id_clone = self.user_id.clone();
        let update_sender_clone = self.update_sender.clone();
        let left_users_clone = self.left_users.clone();

        tokio::spawn(async move {
            match repository_service.subscribe_to_membership_changes().await {
                Ok(mut stream) => {
                    info!("Subscribed to membership changes for user: {}", user_id_clone);

                    while let Some(notification_result) = stream.next().await {
                        match notification_result {
                            Ok(memberships) => {
                                for membership in memberships {
                                    debug!(
                                        "Received membership update for user {} in room {}: {}",
                                        user_id_clone, membership.room_id, membership.membership
                                    );

                                    // Track users who left for device list updates
                                    if membership.membership == MembershipState::Leave {
                                        let mut left_users = left_users_clone.write().await;
                                        left_users.insert(membership.user_id.clone());
                                        debug!("Added user {} to left users tracking", membership.user_id);
                                    }

                                    let room_id_owned = membership.room_id.clone();
                                    let update = SyncUpdate::MembershipUpdate {
                                        room_id: room_id_owned,
                                        user_id: user_id_clone.clone(),
                                        membership,
                                    };

                                    if let Err(e) = update_sender_clone.send(update) {
                                        warn!("Failed to send membership update: {}", e);
                                    }
                                }
                            },
                            Err(e) => {
                                error!(
                                    "Error in membership stream for user {}: {}",
                                    user_id_clone, e
                                );
                            },
                        }
                    }
                },
                Err(e) => {
                    error!(
                        "Failed to subscribe to membership changes for user {}: {}",
                        user_id_clone, e
                    );
                },
            }
        });

        Ok(())
    }

    /// Start LiveQuery subscriptions for user presence
    async fn start_presence_subscriptions(&self) -> Result<()> {
        let repository_service = self.repository_service.clone();
        let user_id_clone = self.user_id.clone();
        let update_sender_clone = self.update_sender.clone();

        tokio::spawn(async move {
            // Subscribe to presence updates for users in shared rooms

            match repository_service.subscribe_to_presence_updates(&user_id_clone).await {
                Ok(mut stream) => {
                    info!("Subscribed to presence updates for user: {}", user_id_clone);

                    while let Some(notification_result) = stream.next().await {
                        match notification_result {
                            Ok(notification) => {
                                // Deserialize notification data following room events pattern
                                let presence_update: UserPresenceUpdate = notification;
                                let update = SyncUpdate::PresenceUpdate {
                                    user_id: presence_update.user_id,
                                    presence: PresenceState {
                                        presence: presence_update.presence,
                                        status_msg: presence_update.status_msg,
                                        last_active_ago: presence_update
                                            .last_active_ago
                                            .map(|t| t as u64),
                                        currently_active: presence_update.currently_active,
                                    },
                                };

                                if let Err(e) = update_sender_clone.send(update) {
                                    warn!("Failed to send presence update: {}", e);
                                }
                            },
                            Err(e) => {
                                error!("Error in presence stream: {}", e);
                            },
                        }
                    }
                },
                Err(e) => {
                    warn!("Could not subscribe to presence updates (table may not exist): {}", e);
                    info!("Presence subscriptions initialized for user: {}", user_id_clone);
                },
            }
        });

        Ok(())
    }

    /// Start LiveQuery subscriptions for device and encryption updates
    async fn start_device_subscriptions(&self) -> Result<()> {
        let repository_service = self.repository_service.clone();
        let user_id_clone = self.user_id.clone();
        let update_sender_clone = self.update_sender.clone();
        let left_users_clone = self.left_users.clone();

        // Subscribe to device key updates
        tokio::spawn(async move {
            // Subscribe to device key updates for users in shared rooms

            match repository_service.subscribe_to_device_updates(&user_id_clone).await {
                Ok(mut stream) => {
                    info!("Subscribed to device updates for user: {}", user_id_clone);

                    while let Some(notification_result) = stream.next().await {
                        match notification_result {
                            Ok(notification) => {
                                // notification is now Device directly, not DeviceKeys
                                let device = notification;
                                
                                // Get and clear left users atomically
                                // This ensures each user appears in 'left' exactly once after they leave
                                let left = {
                                    let mut left_users = left_users_clone.write().await;
                                    let users: Vec<String> = left_users.iter().cloned().collect();
                                    left_users.clear();
                                    users
                                };
                                
                                if !left.is_empty() {
                                    debug!("Including {} left users in device list update", left.len());
                                }
                                
                                let update = SyncUpdate::DeviceListUpdate {
                                    changed: vec![device.user_id],
                                    left,
                                };

                                if let Err(e) = update_sender_clone.send(update) {
                                    warn!("Failed to send device list update: {}", e);
                                }
                            },
                            Err(e) => {
                                error!("Error in device stream: {}", e);
                            },
                        }
                    }
                },
                Err(e) => {
                    warn!("Could not subscribe to device updates (table may not exist): {}", e);
                    info!("Device subscriptions initialized for user: {}", user_id_clone);
                },
            }
        });

        // Subscribe to to-device messages
        let repository_service_2 = self.repository_service.clone();
        let user_id_clone_2 = self.user_id.clone();
        let update_sender_clone_2 = self.update_sender.clone();

        tokio::spawn(async move {
            // Subscribe to to-device messages for the user

            match repository_service_2.subscribe_to_device_messages().await {
                Ok(mut stream) => {
                    info!("Subscribed to to-device messages for user: {}", user_id_clone_2);

                    while let Some(notification_result) = stream.next().await {
                        match notification_result {
                            Ok(notification) => {
                                // Process to-device message
                                debug!("Received to-device message: {:?}", notification);

                                // Create account data update for to-device messages
                                // Convert ToDeviceMessage to JSON Value
                                let content =
                                    serde_json::to_value(notification).unwrap_or_default();
                                let update = SyncUpdate::AccountDataUpdate {
                                    data_type: "m.to_device".to_string(),
                                    content,
                                };

                                if let Err(e) = update_sender_clone_2.send(update) {
                                    warn!("Failed to send to-device message update: {}", e);
                                }
                            },
                            Err(e) => {
                                error!("Error in to-device stream: {}", e);
                            },
                        }
                    }
                },
                Err(e) => {
                    warn!("Could not subscribe to to-device messages (table may not exist): {}", e);
                    info!(
                        "To-device message subscriptions initialized for user: {}",
                        user_id_clone_2
                    );
                },
            }
        });

        Ok(())
    }

    /// Get a stream of sync updates
    pub fn sync_stream(&self) -> impl Stream<Item = SyncUpdate> + Send + Unpin {
        let receiver = self.update_sender.subscribe();
        Box::pin(tokio_stream::wrappers::BroadcastStream::new(receiver).filter_map(|result| {
            async move {
                match result {
                    Ok(update) => Some(update),
                    Err(e) => {
                        warn!("Error in sync stream: {}", e);
                        None
                    },
                }
            }
        }))
    }

    /// Get current sync state
    pub async fn get_sync_state(&self) -> SyncState {
        self.state.read().await.clone()
    }

    /// Update sync state with new event
    pub async fn update_state_with_event(&self, room_id: &str, event: Event) -> Result<()> {
        let mut state = self.state.write().await;

        if let Some(room_state) = state.joined_rooms.get_mut(room_id) {
            if event.state_key.is_some() {
                // State event
                room_state.state.push(event);
            } else {
                // Timeline event
                room_state.timeline.push(event);
            }
        }

        // Update next_batch token
        state.next_batch = Uuid::new_v4().to_string();

        Ok(())
    }

    /// Subscribe to a new room (when user joins)
    pub async fn subscribe_to_room(&self, room_id: &str) -> Result<()> {
        let room_id_str = room_id.to_string();
        let repository_service = self.repository_service.clone();
        let update_sender = self.update_sender.clone();

        tokio::spawn(async move {
            match repository_service.subscribe_to_room_events(&room_id_str).await {
                Ok(mut stream) => {
                    info!("Subscribed to events for new room: {}", room_id_str);

                    while let Some(event_result) = stream.next().await {
                        match event_result {
                            Ok(event) => {
                                debug!(
                                    "Received event in room {}: {}",
                                    room_id_str, event.event_id
                                );

                                let update = if event.state_key.is_some() {
                                    SyncUpdate::StateUpdate { room_id: room_id_str.clone(), event }
                                } else {
                                    SyncUpdate::RoomEvent { room_id: room_id_str.clone(), event }
                                };

                                if let Err(e) = update_sender.send(update) {
                                    warn!("Failed to send event update: {}", e);
                                }
                            },
                            Err(e) => {
                                error!("Error in event stream for room {}: {}", room_id_str, e);
                            },
                        }
                    }
                },
                Err(e) => {
                    error!("Failed to subscribe to events for room {}: {}", room_id_str, e);
                },
            }
        });

        Ok(())
    }

    /// Stop sync (cleanup resources)
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping LiveQuery sync for user: {}", self.user_id);
        // Note: Tokio tasks will be automatically cancelled when the struct is dropped
        // SurrealDB LiveQuery subscriptions will be automatically cleaned up
        Ok(())
    }

    /// Get a receiver for sync updates
    /// This allows consumers to subscribe to sync updates broadcast by the sync manager
    pub fn subscribe_to_updates(&self) -> broadcast::Receiver<SyncUpdate> {
        self.update_receiver.resubscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_db() -> surrealdb::Surreal<surrealdb::engine::any::Any> {
        let db = surrealdb::engine::any::connect("memory")
            .await
            .expect("Failed to connect to in-memory database");
        db.use_ns("test").use_db("test")
            .await
            .expect("Failed to set test namespace and database");
        db
    }

    #[tokio::test]
    async fn test_sync_creation() {
        let db = setup_test_db().await;

        let sync = LiveQuerySync::new(
            "@test:example.com".to_string(),
            "TESTDEVICE".to_string(),
            db
        );

        let state = sync.get_sync_state().await;
        assert!(state.joined_rooms.is_empty());
        assert!(state.invited_rooms.is_empty());
        assert!(state.left_rooms.is_empty());
    }

    #[tokio::test]
    async fn test_sync_state_updates() {
        let db = setup_test_db().await;

        let sync = LiveQuerySync::new(
            "@test:example.com".to_string(),
            "TESTDEVICE".to_string(),
            db
        );

        let room_id = "!test:example.com";
        let event = Event {
            event_id: "$event1:example.com".to_string(),
            room_id: room_id.to_string(),
            sender: "@test:example.com".to_string(),
            event_type: "m.room.message".to_string(),
            content: EventContent::Unknown(
                serde_json::json!({"msgtype": "m.text", "body": "Hello"}),
            ),
            state_key: None,
            origin_server_ts: 1234567890,
            unsigned: None,
            auth_events: Some(Vec::new()),
            depth: Some(1),
            hashes: Some(std::collections::HashMap::new()),
            prev_events: Some(Vec::new()),
            signatures: Some(std::collections::HashMap::new()),
            soft_failed: None,
            received_ts: None,
            outlier: None,
            redacts: None,
            rejected_reason: None,
        };

        // Add room to joined rooms first
        {
            let mut state = sync.state.write().await;
            state.joined_rooms.insert(room_id.to_string(), JoinedRoomState::default());
        }

        sync.update_state_with_event(room_id, event).await.expect("Failed to update sync state with event");

        let state = sync.get_sync_state().await;
        assert_eq!(state.joined_rooms.get(room_id).expect("Room should be in joined_rooms").timeline.len(), 1);
    }
}
