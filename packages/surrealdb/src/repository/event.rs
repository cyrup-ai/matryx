use crate::repository::error::RepositoryError;
use futures::{Stream, StreamExt, TryStreamExt};
use matryx_entity::types::Event;
use surrealdb::{Connection, Surreal};

pub struct EventRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> EventRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    pub async fn create(&self, event: &Event) -> Result<Event, RepositoryError> {
        let event_clone = event.clone();
        let created: Option<Event> =
            self.db.create(("event", &event.event_id)).content(event_clone).await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create event"))
        })
    }

    pub async fn get_by_id(&self, event_id: &str) -> Result<Option<Event>, RepositoryError> {
        let event: Option<Event> = self.db.select(("event", event_id)).await?;
        Ok(event)
    }

    pub async fn get_room_events(
        &self,
        room_id: &str,
        limit: Option<u32>,
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = match limit {
            Some(l) => {
                format!(
                    "SELECT * FROM event WHERE room_id = $room_id ORDER BY origin_server_ts DESC LIMIT {}",
                    l
                )
            },
            None => {
                "SELECT * FROM event WHERE room_id = $room_id ORDER BY origin_server_ts DESC"
                    .to_string()
            },
        };

        let room_id_owned = room_id.to_string();
        let events: Vec<Event> =
            self.db.query(&query).bind(("room_id", room_id_owned)).await?.take(0)?;
        Ok(events)
    }

    pub async fn get_state_events(&self, room_id: &str) -> Result<Vec<Event>, RepositoryError> {
        let room_id_owned = room_id.to_string();
        let events: Vec<Event> = self
            .db
            .query("SELECT * FROM event WHERE room_id = $room_id AND state_key IS NOT NULL")
            .bind(("room_id", room_id_owned))
            .await?
            .take(0)?;
        Ok(events)
    }

    /// Subscribe to real-time room events using SurrealDB LiveQuery
    /// Returns a stream of notifications for new events in the specified room
    pub async fn subscribe_room_events(
        &self,
        room_id: &str,
    ) -> Result<impl Stream<Item = Result<Event, RepositoryError>>, RepositoryError> {
        // Create SurrealDB LiveQuery for events in specific room (message events only)
        let mut stream = self
            .db
            .query("LIVE SELECT * FROM event WHERE room_id = $room_id AND state_key IS NULL")
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        // Transform SurrealDB notification stream to event stream
        let event_stream = stream
            .stream::<surrealdb::Notification<Event>>(0)
            .map_err(RepositoryError::Database)?
            .map(|notification_result| -> Result<Event, RepositoryError> {
                let notification = notification_result.map_err(RepositoryError::Database)?;

                match notification.action {
                    surrealdb::Action::Create | surrealdb::Action::Update => Ok(notification.data),
                    surrealdb::Action::Delete => {
                        // For deleted events, we still return the event data
                        // so consumers can handle deletion/redaction appropriately
                        Ok(notification.data)
                    },
                    _ => {
                        // Handle any future Action variants
                        Ok(notification.data)
                    },
                }
            });

        Ok(event_stream)
    }

    /// Subscribe to real-time room state events using SurrealDB LiveQuery
    /// Returns a stream of notifications for state changes in the specified room
    pub async fn subscribe_room_state_events(
        &self,
        room_id: &str,
    ) -> Result<impl Stream<Item = Result<Event, RepositoryError>>, RepositoryError> {
        // Create SurrealDB LiveQuery for state events in specific room
        let mut stream = self
            .db
            .query("LIVE SELECT * FROM event WHERE room_id = $room_id AND state_key IS NOT NULL")
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        // Transform SurrealDB notification stream to event stream
        let event_stream = stream
            .stream::<surrealdb::Notification<Event>>(0)
            .map_err(RepositoryError::Database)?
            .map(|notification_result| -> Result<Event, RepositoryError> {
                let notification = notification_result.map_err(RepositoryError::Database)?;

                match notification.action {
                    surrealdb::Action::Create | surrealdb::Action::Update => Ok(notification.data),
                    surrealdb::Action::Delete => {
                        // For deleted state events, return the data for proper state resolution
                        Ok(notification.data)
                    },
                    _ => {
                        // Handle any future Action variants
                        Ok(notification.data)
                    },
                }
            });

        Ok(event_stream)
    }

    /// Subscribe to all events for a specific user across all rooms they have access to
    /// Returns a stream of notifications for events the user can see
    pub async fn subscribe_user_events(
        &self,
        user_id: &str,
    ) -> Result<impl Stream<Item = Result<Event, RepositoryError>>, RepositoryError> {
        // Create SurrealDB LiveQuery for events in rooms where user has membership
        let mut stream = self
            .db
            .query(
                r#"
                LIVE SELECT * FROM event 
                WHERE room_id IN (
                    SELECT VALUE room_id FROM membership 
                    WHERE user_id = $user_id AND membership IN ['join', 'invite']
                )
            "#,
            )
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(RepositoryError::Database)?;

        // Transform SurrealDB notification stream to event stream
        let event_stream = stream
            .stream::<surrealdb::Notification<Event>>(0)
            .map_err(RepositoryError::Database)?
            .map(|notification_result| -> Result<Event, RepositoryError> {
                let notification = notification_result.map_err(RepositoryError::Database)?;

                match notification.action {
                    surrealdb::Action::Create | surrealdb::Action::Update => Ok(notification.data),
                    surrealdb::Action::Delete => {
                        // For deleted events, return data for proper handling
                        Ok(notification.data)
                    },
                    _ => {
                        // Handle any future Action variants
                        Ok(notification.data)
                    },
                }
            });

        Ok(event_stream)
    }
}
