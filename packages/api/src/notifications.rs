//! Notifications settings wrapper with synchronous interfaces
//!
//! This module provides a clean, synchronous interface to Matrix SDK's Notification Settings
//! that eliminates the need for async_trait and Box<dyn Future> in client code.

use std::sync::Arc;
use tokio::runtime::Handle;

use matrix_sdk::{
    notification_settings::{
        NotificationSettings as MatrixNotificationSettings,
        RoomNotificationMode,
    },
    ruma::{OwnedRoomId, RoomId},
    Client as MatrixClient,
};

use crate::error::NotificationError;
use crate::future::MatrixFuture;

/// A synchronous wrapper around the Matrix SDK Notification Settings.
///
/// This wrapper enables using the NotificationSettings with a synchronous interface,
/// hiding all async complexity behind MatrixFuture objects that properly
/// implement the Future trait.
pub struct MatrixNotifications {
    client: Arc<MatrixClient>,
    inner: Arc<MatrixNotificationSettings>,
    runtime_handle: Handle,
}

impl MatrixNotifications {
    /// Create a new MatrixNotifications with the provided Matrix client.
    pub fn new(client: Arc<MatrixClient>) -> MatrixFuture<Self> {
        let client_clone = client.clone();

        MatrixFuture::spawn(async move {
            let settings = client_clone.notification_settings().await;

            Ok(Self {
                client: client_clone,
                inner: Arc::new(settings),
                runtime_handle: Handle::current(),
            })
        })
    }

    /// Rebuild the notification settings.
    ///
    /// This is needed after making changes to ensure settings are up to date.
    pub fn rebuild(&mut self) -> MatrixFuture<()> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // Still need to get the settings, but we won't use them directly here
            // The caller will need to recreate the MatrixNotifications instance
            let _settings = client.notification_settings().await;
            Ok(())
        })
    }

    /// Get the notification mode for a room.
    pub fn get_room_notification_mode(
        &self,
        room_id: &RoomId,
    ) -> MatrixFuture<Option<RoomNotificationMode>> {
        let room_id = room_id.to_owned();
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            let mode = inner.get_user_defined_room_notification_mode(&room_id).await;
            Ok(mode)
        })
    }

    /// Set the notification mode for a room.
    pub fn set_room_notification_mode(
        &self,
        room_id: &RoomId,
        mode: RoomNotificationMode,
    ) -> MatrixFuture<()> {
        let room_id = room_id.to_owned();
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            inner
                .set_room_notification_mode(&room_id, mode)
                .await
                .map_err(|e| crate::error::Error::Notification(NotificationError::matrix_sdk(e)))?;
            Ok(())
        })
    }

    /// Reset the notification mode for a room (removing any user-defined settings).
    pub fn reset_room_notification_mode(&self, room_id: &RoomId) -> MatrixFuture<()> {
        let room_id = room_id.to_owned();
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            inner
                .delete_user_defined_room_rules(&room_id)
                .await
                .map_err(|e| crate::error::Error::Notification(NotificationError::matrix_sdk(e)))?;
            Ok(())
        })
    }

    /// Check if a room is muted.
    pub fn is_room_muted(&self, room_id: &RoomId) -> MatrixFuture<bool> {
        let room_id = room_id.to_owned();
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            // In Matrix SDK 0.10.0, we need to check if the room notification mode is set to Mute
            // The is_room_muted method doesn't exist anymore
            let mode = inner.get_user_defined_room_notification_mode(&room_id).await;
            Ok(matches!(mode, Some(RoomNotificationMode::Mute)))
        })
    }

    /// Set whether a room is muted.
    pub fn set_room_muted(&self, room_id: &RoomId, muted: bool) -> MatrixFuture<()> {
        let room_id = room_id.to_owned();
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            if muted {
                inner
                    .set_room_notification_mode(&room_id, RoomNotificationMode::Mute)
                    .await
                    .map_err(|e| {
                        crate::error::Error::Notification(NotificationError::matrix_sdk(e))
                    })?;
                Ok(())
            } else {
                // If the room is currently muted, set to default (remove rule)
                let current_mode = inner.get_user_defined_room_notification_mode(&room_id).await;
                if let Some(RoomNotificationMode::Mute) = current_mode {
                    inner.delete_user_defined_room_rules(&room_id).await.map_err(|e| {
                        crate::error::Error::Notification(NotificationError::matrix_sdk(e))
                    })?;
                }
                Ok(()) // Already not muted or now unmuted
            }
        })
    }

    /// Get the content for user notification settings.
    pub fn get_user_notification_content(&self) -> MatrixFuture<String> {
        let client = self.client.clone();

        MatrixFuture::spawn(async move {
            // In Matrix SDK 0.10.0, we use push rules content type directly
            use matrix_sdk::ruma::events::push_rules::PushRulesEventContent;

            let raw_account_data = client
                .account()
                .account_data::<PushRulesEventContent>()
                .await
                .map_err(|e| {
                crate::error::Error::Notification(NotificationError::matrix_sdk(e))
            })?;

            if let Some(content) = raw_account_data {
                // Serialize the content to a string
                let json = serde_json::to_string(&content).map_err(|e| {
                    crate::error::Error::Notification(NotificationError::SerializationError(
                        e.to_string(),
                    ))
                })?;
                Ok(json)
            } else {
                // If there's no push rules event, return an empty JSON object
                Ok("{}".to_string())
            }
        })
    }

    /// Get a list of rooms with user-defined notification settings.
    pub fn get_rooms_with_user_defined_rules(&self) -> MatrixFuture<Vec<OwnedRoomId>> {
        let inner = self.inner.clone();

        MatrixFuture::spawn(async move {
            // In Matrix SDK 0.10.0, get_rooms_with_user_defined_rules requires a parameter
            let rule_rooms = inner.get_rooms_with_user_defined_rules(None).await;

            // Convert from String to OwnedRoomId
            let mut rooms = Vec::new();
            for room_str in rule_rooms {
                match OwnedRoomId::try_from(room_str) {
                    Ok(room_id) => rooms.push(room_id),
                    Err(e) => eprintln!("Invalid room ID: {}", e),
                }
            }

            Ok(rooms)
        })
    }
}
