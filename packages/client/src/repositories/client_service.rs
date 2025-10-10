use futures_util::{Stream, StreamExt};
use matryx_entity::{Device, Event, Membership, UserPresenceUpdate};
use matryx_surrealdb::repository::{
    DeviceRepository,
    EventRepository,
    MembershipRepository,
    PresenceRepository,
    RepositoryError,
    ToDeviceMessage,
    ToDeviceRepository,
};
use std::pin::Pin;

use surrealdb::{Surreal, engine::any::Any};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),
    #[error("Database error: {0}")]
    Database(#[from] surrealdb::Error),
    #[error("Authentication error: {0}")]
    Authentication(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncUpdate {
    pub events: Vec<Event>,
    pub membership_changes: Vec<Membership>,
    pub presence_updates: Vec<UserPresenceUpdate>,
    pub device_updates: Vec<Device>,
    pub to_device_messages: Vec<ToDeviceMessage>,
}

/// Coordinated client service that properly uses server repositories
#[derive(Clone)]
pub struct ClientRepositoryService {
    event_repo: EventRepository,
    membership_repo: MembershipRepository,
    presence_repo: PresenceRepository,
    device_repo: DeviceRepository,
    to_device_repo: ToDeviceRepository,
    user_id: String,
    device_id: String,
}

impl ClientRepositoryService {
    pub fn new(
        event_repo: EventRepository,
        membership_repo: MembershipRepository,
        presence_repo: PresenceRepository,
        device_repo: DeviceRepository,
        to_device_repo: ToDeviceRepository,
        user_id: String,
        device_id: String,
    ) -> Self {
        Self {
            event_repo,
            membership_repo,
            presence_repo,
            device_repo,
            to_device_repo,
            user_id,
            device_id,
        }
    }

    /// Create service from database connection
    pub fn from_db(db: Surreal<Any>, user_id: String, device_id: String) -> Self {
        let event_repo = EventRepository::new(db.clone());
        let membership_repo = MembershipRepository::new(db.clone());
        let presence_repo = PresenceRepository::new(db.clone());
        let device_repo = DeviceRepository::new(db.clone());
        let to_device_repo = ToDeviceRepository::new(db);

        Self::new(event_repo, membership_repo, presence_repo, device_repo, to_device_repo, user_id, device_id)
    }

    /// Get room events using proper repository pattern
    pub async fn get_room_events(
        &self,
        room_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Event>, ClientError> {
        let events = self.event_repo.get_room_events(room_id, limit).await?;
        Ok(events)
    }

    /// Get user memberships using proper repository pattern
    pub async fn get_user_memberships(&self) -> Result<Vec<Membership>, ClientError> {
        let memberships = self.membership_repo.get_user_rooms(&self.user_id).await?;
        Ok(memberships)
    }

    /// Get current user presence
    pub async fn get_user_presence(
        &self,
        user_id: &str,
    ) -> Result<Option<UserPresenceUpdate>, ClientError> {
        let presence = self.presence_repo.get_user_presence(user_id).await?;
        Ok(presence)
    }

    /// Get user devices
    pub async fn get_user_devices(&self, user_id: &str) -> Result<Vec<Device>, ClientError> {
        let devices = self.device_repo.get_user_devices(user_id).await?;
        Ok(devices)
    }

    /// Get to-device messages for user and device
    pub async fn get_to_device_messages(
        &self,
        device_id: &str,
        _limit: Option<u32>,
    ) -> Result<Vec<ToDeviceMessage>, ClientError> {
        let messages = self
            .to_device_repo
            .get_to_device_messages(&self.user_id, device_id, None)
            .await?;
        Ok(messages)
    }

    /// Subscribe to room events
    pub async fn subscribe_to_room_events(
        &self,
        room_id: &str,
    ) -> Result<impl Stream<Item = Result<Event, ClientError>>, ClientError> {
        let stream = self
            .event_repo
            .subscribe_room_events(room_id)
            .await?
            .map(|result| result.map_err(ClientError::Repository));

        Ok(stream)
    }

    /// Subscribe to user events across all rooms
    pub async fn subscribe_to_user_events(
        &self,
    ) -> Result<impl Stream<Item = Result<Event, ClientError>>, ClientError> {
        let stream = self
            .event_repo
            .subscribe_user_events(&self.user_id)
            .await?
            .map(|result| result.map_err(ClientError::Repository));

        Ok(stream)
    }

    /// Subscribe to device list updates for the current user
    pub async fn subscribe_to_device_updates<'a>(
        &'a self,
        _user_id: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Device, ClientError>> + Send + 'a>>, ClientError>
    {
        // Use the device repository to subscribe to device updates
        let stream = self
            .device_repo
            .subscribe_to_device_keys(&self.user_id)
            .await?
            .map(|result| result.map_err(ClientError::Repository));

        Ok(Box::pin(stream))
    }

    /// Subscribe to to-device messages for the current user
    ///
    /// Creates a SurrealDB LIVE query stream for real-time to-device message delivery.
    /// Messages are delivered as they arrive and should be acknowledged after processing.
    ///
    /// # Returns
    /// A stream of to-device messages or errors
    ///
    /// # Errors
    /// Returns `ClientError` if the subscription cannot be created
    pub async fn subscribe_to_device_messages<'a>(
        &'a self,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<ToDeviceMessage, ClientError>> + Send + 'a>>,
        ClientError,
    > {
        tracing::debug!(
            "Subscribing to to-device messages for user {} device {}",
            self.user_id,
            self.device_id
        );

        // Call the repository's subscription method
        let stream = self
            .to_device_repo
            .subscribe_to_device_messages(&self.user_id, &self.device_id)
            .await?
            .map(|result| result.map_err(ClientError::Repository));

        Ok(Box::pin(stream))
    }

    /// Mark to-device messages as delivered
    ///
    /// Should be called after successfully processing to-device messages.
    /// This allows the server to clean up delivered messages.
    ///
    /// # Arguments
    /// * `message_ids` - List of message IDs to acknowledge
    ///
    /// # Errors
    /// Returns `ClientError` if the acknowledgment fails
    pub async fn acknowledge_to_device_messages(
        &self,
        message_ids: &[String],
    ) -> Result<(), ClientError> {
        tracing::debug!("Acknowledging {} to-device messages", message_ids.len());

        self.to_device_repo
            .mark_to_device_messages_delivered(&self.user_id, &self.device_id, message_ids)
            .await?;

        Ok(())
    }

    /// Subscribe to membership changes for the current user  
    pub async fn subscribe_to_membership_changes<'a>(
        &'a self,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<Vec<Membership>, ClientError>> + Send + 'a>>,
        ClientError,
    > {
        // Use the membership repository to subscribe to changes
        let stream = self.membership_repo.subscribe_user_membership(&self.user_id).await?.map(
            |result| -> Result<Vec<Membership>, ClientError> {
                let membership = result.map_err(ClientError::Repository)?;
                // Convert single membership to vec for compatibility
                Ok(vec![membership])
            },
        );

        Ok(Box::pin(stream))
    }

    /// Subscribe to presence updates for a user
    pub async fn subscribe_to_presence_updates<'a>(
        &'a self,
        user_id: &'a str,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<UserPresenceUpdate, ClientError>> + Send + 'a>>,
        ClientError,
    > {
        let stream = self
            .presence_repo
            .subscribe_to_user_presence(user_id)
            .await?
            .map(|result| result.map_err(ClientError::Repository));

        Ok(Box::pin(stream))
    }

    /// Get sync updates as a snapshot instead of subscription
    /// This replaces the subscription-based approach with a simpler polling approach
    pub async fn get_sync_updates(&self) -> Result<SyncUpdate, ClientError> {
        // Get current state from repositories
        let events = vec![]; // TODO: Implement user events aggregation when needed
        let membership_changes = self.membership_repo.get_user_rooms(&self.user_id).await?;
        let presence_updates = vec![]; // TODO: Implement when presence repository has the method
        let device_updates = self.device_repo.get_user_devices(&self.user_id).await?;
        let to_device_messages = self
            .to_device_repo
            .get_to_device_messages(&self.user_id, "", None)
            .await
            .unwrap_or_default();

        Ok(SyncUpdate {
            events,
            membership_changes,
            presence_updates,
            device_updates,
            to_device_messages,
        })
    }

    /// Verify user authentication and access
    pub async fn verify_user_access(&self, room_id: &str) -> Result<bool, ClientError> {
        // Check if user is a member of the room
        let is_member = self.membership_repo.is_user_in_room(room_id, &self.user_id).await?;
        Ok(is_member)
    }

    /// Get the authenticated user ID
    pub fn get_user_id(&self) -> &str {
        &self.user_id
    }
}
