use matryx_entity::{Event, EventContent};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::AppState;

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
}

pub struct ReplacementValidator;

impl ReplacementValidator {
    pub fn new() -> Self {
        Self
    }

    pub async fn validate_replacement(
        &self,
        original_event_id: &str,
        replacement_event: &Value,
        sender: &str,
        state: &AppState,
    ) -> Result<(), ReplacementError> {
        info!("Validating replacement for event {} by sender {}", original_event_id, sender);

        // Get original event
        let original_event = self.get_event(original_event_id, state).await?;

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
        if !replacement_event["content"]["m.new_content"].is_object() {
            warn!("Replacement event missing m.new_content");
            return Err(ReplacementError::MissingNewContent);
        }

        // Validate m.relates_to structure
        let relates_to = &replacement_event["content"]["m.relates_to"];
        if relates_to["rel_type"].as_str() != Some("m.replace") {
            warn!("Invalid rel_type for replacement event");
            return Err(ReplacementError::MissingNewContent);
        }

        if relates_to["event_id"].as_str() != Some(original_event_id) {
            warn!("Replacement relates_to event_id doesn't match original");
            return Err(ReplacementError::MissingNewContent);
        }

        info!("Replacement validation successful for event {}", original_event_id);
        Ok(())
    }

    pub async fn apply_replacement(
        &self,
        original_event_id: &str,
        replacement_event: &Value,
        state: &AppState,
    ) -> Result<(), ReplacementError> {
        info!("Applying replacement for event {}", original_event_id);

        let replacement_event_id = replacement_event["event_id"]
            .as_str()
            .ok_or(ReplacementError::MissingNewContent)?;
        let room_id = replacement_event["room_id"]
            .as_str()
            .ok_or(ReplacementError::MissingNewContent)?;
        let sender = replacement_event["sender"]
            .as_str()
            .ok_or(ReplacementError::MissingNewContent)?;

        // Store replacement in relations table
        let query = "
            CREATE event_relations SET
                event_id = $replacement_event_id,
                relates_to_event_id = $original_event_id,
                rel_type = 'm.replace',
                room_id = $room_id,
                sender = $sender,
                created_at = time::now()
        ";

        let result = state
            .db
            .query(query)
            .bind(("replacement_event_id", replacement_event_id.to_string()))
            .bind(("original_event_id", original_event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .await
            .map_err(|e| ReplacementError::DatabaseError(e.to_string()))?;

        // Store edit history
        self.store_edit_history(original_event_id, replacement_event_id, state)
            .await?;

        info!("Successfully applied replacement for event {}", original_event_id);
        Ok(())
    }

    pub async fn get_replacement_history(
        &self,
        original_event_id: &str,
        state: &AppState,
    ) -> Result<Vec<Event>, ReplacementError> {
        let query = "
            SELECT e.* FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $original_event_id 
            AND r.rel_type = 'm.replace'
            ORDER BY e.origin_server_ts ASC
        ";

        let mut result = state
            .db
            .query(query)
            .bind(("original_event_id", original_event_id.to_string()))
            .await
            .map_err(|e| ReplacementError::DatabaseError(e.to_string()))?;

        let events: Vec<Value> = result
            .take(0)
            .map_err(|e| ReplacementError::DatabaseError(e.to_string()))?;

        let mut replacement_events = Vec::new();
        for event_data in events {
            if let Ok(event) = serde_json::from_value::<Event>(event_data) {
                replacement_events.push(event);
            }
        }

        Ok(replacement_events)
    }

    pub async fn get_latest_replacement(
        &self,
        original_event_id: &str,
        state: &AppState,
    ) -> Result<Option<Event>, ReplacementError> {
        let history = self.get_replacement_history(original_event_id, state).await?;
        Ok(history.last().cloned())
    }

    async fn get_event(&self, event_id: &str, state: &AppState) -> Result<Event, ReplacementError> {
        let query = "
            SELECT event_id, room_id, sender, content, origin_server_ts, type
            FROM room_timeline_events 
            WHERE event_id = $event_id
            LIMIT 1
        ";

        let mut result = state
            .db
            .query(query)
            .bind(("event_id", event_id.to_string()))
            .await
            .map_err(|e| ReplacementError::DatabaseError(e.to_string()))?;

        let events: Vec<Value> = result
            .take(0)
            .map_err(|e| ReplacementError::DatabaseError(e.to_string()))?;

        if let Some(event_data) = events.first() {
            let content = serde_json::from_value::<EventContent>(event_data["content"].clone())
                .unwrap_or_default();

            let event = Event::new(
                event_data["event_id"].as_str().unwrap_or("").to_string(),
                event_data["sender"].as_str().unwrap_or("").to_string(),
                event_data["origin_server_ts"].as_i64().unwrap_or(0),
                event_data["type"].as_str().unwrap_or("").to_string(),
                event_data["room_id"].as_str().unwrap_or("").to_string(),
                content,
            );
            Ok(event)
        } else {
            Err(ReplacementError::OriginalEventNotFound)
        }
    }

    async fn store_edit_history(
        &self,
        original_event_id: &str,
        replacement_event_id: &str,
        state: &AppState,
    ) -> Result<(), ReplacementError> {
        let query = "
            CREATE edit_history SET
                id = rand::uuid(),
                original_event_id = $original_event_id,
                replacement_event_id = $replacement_event_id,
                created_at = time::now()
        ";

        state
            .db
            .query(query)
            .bind(("original_event_id", original_event_id.to_string()))
            .bind(("replacement_event_id", replacement_event_id.to_string()))
            .await
            .map_err(|e| ReplacementError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}

impl Default for ReplacementValidator {
    fn default() -> Self {
        Self::new()
    }
}
