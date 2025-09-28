use serde_json::Value;
use surrealdb::{Connection, Surreal};
use tracing::{info, warn};

use crate::repository::error::RepositoryError;
use matryx_entity::types::Event;

/// Repository for managing event replacements (Matrix m.replace relations)
pub struct EventReplacementRepository<C: Connection> {
    db: Surreal<C>,
}

impl<C: Connection> EventReplacementRepository<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self { db }
    }

    /// Apply a replacement relationship between events
    pub async fn apply_replacement(
        &self,
        original_event_id: &str,
        replacement_event_id: &str,
        room_id: &str,
        sender: &str,
    ) -> Result<(), RepositoryError> {
        info!("Applying replacement for event {}", original_event_id);

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

        let mut result = self
            .db
            .query(query)
            .bind(("replacement_event_id", replacement_event_id.to_string()))
            .bind(("original_event_id", original_event_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("sender", sender.to_string()))
            .await?;

        // Verify the replacement operation succeeded
        let _: Option<Value> = result.take(0)?;
        
        info!("Successfully applied replacement {} -> {}", original_event_id, replacement_event_id);
        Ok(())
    }

    /// Get replacement history for an event (chronological order)
    pub async fn get_replacement_history(
        &self,
        original_event_id: &str,
    ) -> Result<Vec<Event>, RepositoryError> {
        let query = "
            SELECT e.* FROM event e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $original_event_id
            AND r.rel_type = 'm.replace'
            ORDER BY e.origin_server_ts ASC
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("original_event_id", original_event_id.to_string()))
            .await?;

        let events: Vec<Event> = result.take(0)?;
        
        if events.is_empty() {
            warn!("No replacement history found for event {}", original_event_id);
        }
        
        Ok(events)
    }

    /// Get the latest replacement for an event
    pub async fn get_latest_replacement(
        &self,
        event_id: &str,
    ) -> Result<Option<Event>, RepositoryError> {
        let query = "
            SELECT e.* FROM event e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $event_id
            AND r.rel_type = 'm.replace'
            ORDER BY e.origin_server_ts DESC
            LIMIT 1
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("event_id", event_id.to_string()))
            .await?;

        let events: Vec<Event> = result.take(0)?;
        Ok(events.into_iter().next())
    }

    /// Remove a replacement relationship
    pub async fn remove_replacement(
        &self,
        original_event_id: &str,
        replacement_event_id: &str,
    ) -> Result<(), RepositoryError> {
        let query = "
            DELETE event_relations 
            WHERE relates_to_event_id = $original_event_id 
            AND event_id = $replacement_event_id
            AND rel_type = 'm.replace'
        ";

        self.db
            .query(query)
            .bind(("original_event_id", original_event_id.to_string()))
            .bind(("replacement_event_id", replacement_event_id.to_string()))
            .await?;

        Ok(())
    }
}