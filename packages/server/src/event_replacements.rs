use serde_json::Value;
use tracing::{error, info, warn};

use crate::state::AppState;
use matryx_surrealdb::repository::{
    event::EventRepository,
    event_replacement::EventReplacementRepository,
    error::RepositoryError,
};
use matryx_entity::types::Event;

#[derive(Debug, thiserror::Error)]
pub enum ReplacementError {
    #[error("Replacement event must be in same room as original")]
    DifferentRoom,
    #[error("Only the original sender can edit their message")]
    DifferentSender,
    #[error("Replacement event must contain m.new_content")]
    MissingNewContent,
    #[error("User lacks permission to edit this message")]
    InsufficientPermissions,
    #[error("Original event not found")]
    OriginalEventNotFound,
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),
}

pub struct ReplacementValidator {
    event_repo: EventRepository,
    replacement_repo: EventReplacementRepository<surrealdb::engine::any::Any>,
}

impl ReplacementValidator {
    pub fn new(state: &AppState) -> Self {
        Self {
            event_repo: EventRepository::new(state.db.clone()),
            replacement_repo: EventReplacementRepository::new(state.db.clone()),
        }
    }

    /// Validate that a replacement event is allowed
    pub async fn validate_replacement(
        &self,
        original_event_id: &str,
        replacement_event: &Value,
        sender: &str,
    ) -> Result<(), ReplacementError> {
        info!("Validating replacement for event {} by sender {}", original_event_id, sender);

        // Get original event
        let original_event = self.get_event(original_event_id).await?;

        // Validate same room
        let replacement_room_id = replacement_event["room_id"]
            .as_str()
            .ok_or(ReplacementError::MissingNewContent)?;

        if original_event.room_id != replacement_room_id {
            warn!(
                "Replacement event room {} differs from original room {}",
                replacement_room_id, original_event.room_id
            );
            return Err(ReplacementError::DifferentRoom);
        }

        // Validate same sender
        if original_event.sender != sender {
            warn!(
                "Replacement sender {} differs from original sender {}",
                sender, original_event.sender
            );
            return Err(ReplacementError::DifferentSender);
        }

        // Validate m.new_content exists
        if replacement_event
            .get("content")
            .and_then(|c| c.get("m.new_content"))
            .is_none()
        {
            warn!("Replacement event missing m.new_content");
            return Err(ReplacementError::MissingNewContent);
        }

        info!("Replacement validation passed for event {}", original_event_id);
        Ok(())
    }

    /// Apply a replacement (store the relationship)
    pub async fn apply_replacement(
        &self,
        original_event_id: &str,
        replacement_event: &Value,
    ) -> Result<(), ReplacementError> {
        let replacement_event_id = replacement_event["event_id"]
            .as_str()
            .ok_or(ReplacementError::MissingNewContent)?;
        let room_id = replacement_event["room_id"]
            .as_str()
            .ok_or(ReplacementError::MissingNewContent)?;
        let sender = replacement_event["sender"]
            .as_str()
            .ok_or(ReplacementError::MissingNewContent)?;

        self.replacement_repo
            .apply_replacement(original_event_id, replacement_event_id, room_id, sender)
            .await?;

        Ok(())
    }

    /// Get replacement history for an event
    pub async fn get_replacement_history(
        &self,
        original_event_id: &str,
    ) -> Result<Vec<Event>, ReplacementError> {
        let events = self.replacement_repo
            .get_replacement_history(original_event_id)
            .await?;

        Ok(events)
    }

    /// Get the latest replacement for an event
    pub async fn get_latest_replacement(
        &self,
        event_id: &str,
    ) -> Result<Option<Event>, ReplacementError> {
        let event = self.replacement_repo
            .get_latest_replacement(event_id)
            .await?;

        Ok(event)
    }

    /// Helper method to get an event by ID
    async fn get_event(&self, event_id: &str) -> Result<Event, ReplacementError> {
        match self.event_repo.get_by_id(event_id).await? {
            Some(event) => Ok(event),
            None => {
                error!("Original event {} not found", event_id);
                Err(ReplacementError::OriginalEventNotFound)
            }
        }
    }

    /// Remove a replacement relationship
    pub async fn remove_replacement(
        &self,
        original_event_id: &str,
        replacement_event_id: &str,
    ) -> Result<(), ReplacementError> {
        self.replacement_repo
            .remove_replacement(original_event_id, replacement_event_id)
            .await?;

        Ok(())
    }
}

impl Default for ReplacementValidator {
    fn default() -> Self {
        // This is a placeholder - in practice, this should be constructed with proper state
        panic!("ReplacementValidator must be constructed with AppState using new()")
    }
}