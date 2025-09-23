use anyhow::Result;
use futures_util::{Stream, StreamExt};
use matryx_entity::{Event, UserPresenceUpdate};
use matryx_surrealdb::repository::{
    DeviceRepository,
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
}

/// Repository for handling event operations using server-side repositories
#[derive(Clone)]
pub struct EventRepository {
    event_repo: matryx_surrealdb::repository::EventRepository,
    device_repo: DeviceRepository,
    presence_repo: PresenceRepository,
    to_device_repo: ToDeviceRepository,
    user_id: String,
}

impl EventRepository {
    pub fn new(db: Surreal<Any>, user_id: String) -> Self {
        Self {
            event_repo: matryx_surrealdb::repository::EventRepository::new(db.clone()),
            device_repo: DeviceRepository::new(db.clone()),
            presence_repo: PresenceRepository::new(db.clone()),
            to_device_repo: ToDeviceRepository::new(db),
            user_id,
        }
    }

    /// Get room events with optional filters using server repository
    pub async fn get_room_events(
        &self,
        room_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Event>, ClientError> {
        let events = self.event_repo.get_room_timeline(room_id, limit).await?;
        Ok(events)
    }

    /// Subscribe to room events using server repository
    pub async fn subscribe_room_events<'a>(
        &'a self,
        room_id: &'a str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Event, ClientError>> + Send + 'a>>, ClientError>
    {
        let stream = self
            .event_repo
            .subscribe_room_events(room_id)
            .await?
            .map(|result| result.map_err(ClientError::Repository));

        Ok(Box::pin(stream))
    }

    /// Subscribe to all room events for the user using server repository
    pub async fn subscribe_all_room_events<'a>(
        &'a self,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Event, ClientError>> + Send + 'a>>, ClientError>
    {
        let stream = self
            .event_repo
            .subscribe_user_events(&self.user_id)
            .await?
            .map(|result| result.map_err(ClientError::Repository));

        Ok(Box::pin(stream))
    }

    /// Subscribe to presence updates using server repository
    pub async fn subscribe_presence_updates<'a>(
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

    /// Subscribe to device list updates using server repository
    pub async fn subscribe_device_list_updates<'a>(
        &'a self,
        user_id: &'a str,
    ) -> Result<
        Pin<Box<dyn Stream<Item = Result<matryx_entity::Device, ClientError>> + Send + 'a>>,
        ClientError,
    > {
        let stream = self
            .device_repo
            .subscribe_to_device_keys(user_id)
            .await?
            .map(|result| result.map_err(ClientError::Repository));

        Ok(Box::pin(stream))
    }

    /// Get pending to-device messages for sync response (Matrix spec compliant)
    pub async fn get_to_device_messages_for_sync(
        &self,
        device_id: &str,
        since: Option<&str>,
    ) -> Result<Vec<ToDeviceMessage>, ClientError> {
        // Get to-device messages for the user and device as per Matrix sync spec
        let messages = self
            .to_device_repo
            .get_to_device_messages(&self.user_id, device_id, since)
            .await?;
        Ok(messages)
    }
}
