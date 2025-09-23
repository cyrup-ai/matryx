use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::AppState;
use matryx_entity::{Event, ThreadSummary};

#[derive(Debug, thiserror::Error)]
pub enum ThreadError {
    #[error("Thread root cannot be an event with relations")]
    InvalidThreadRoot,
    #[error("Thread event must be in same room as thread root")]
    DifferentRoom,
    #[error("Thread depth exceeds maximum allowed")]
    ExcessiveDepth,
    #[error("Thread root event not found")]
    ThreadRootNotFound,
    #[error("Database error: {0}")]
    DatabaseError(String),
}

pub struct ThreadManager;

impl ThreadManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn validate_thread_event(
        &self,
        thread_root_id: &str,
        thread_event: &Value,
        state: &AppState,
    ) -> Result<(), ThreadError> {
        info!("Validating thread event for root {}", thread_root_id);

        // Get thread root event
        let root_event = self.get_event(thread_root_id, state).await?;

        // Validate root is not itself a relation
        if root_event.content.get("m.relates_to").is_some() {
            warn!("Thread root {} is itself a relation", thread_root_id);
            return Err(ThreadError::InvalidThreadRoot);
        }

        // Validate same room
        let thread_room_id = thread_event["room_id"].as_str().ok_or(ThreadError::DifferentRoom)?;

        if root_event.room_id != thread_room_id {
            warn!(
                "Thread event room {} differs from root room {}",
                thread_room_id, root_event.room_id
            );
            return Err(ThreadError::DifferentRoom);
        }

        // Validate thread depth (prevent excessive nesting)
        self.validate_thread_depth(thread_root_id, state).await?;

        // Validate m.relates_to structure
        let relates_to = &thread_event["content"]["m.relates_to"];
        if relates_to["rel_type"].as_str() != Some("m.thread") {
            warn!("Invalid rel_type for thread event");
            return Err(ThreadError::InvalidThreadRoot);
        }

        if relates_to["event_id"].as_str() != Some(thread_root_id) {
            warn!("Thread relates_to event_id doesn't match root");
            return Err(ThreadError::InvalidThreadRoot);
        }

        info!("Thread validation successful for root {}", thread_root_id);
        Ok(())
    }

    pub async fn get_thread_events(
        &self,
        thread_root_id: &str,
        limit: Option<u32>,
        from: Option<String>,
        state: &AppState,
    ) -> Result<Vec<Event>, ThreadError> {
        state
            .thread_repository
            .get_thread_events(thread_root_id, limit)
            .await
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))
    }

    pub async fn generate_thread_summary(
        &self,
        thread_root_id: &str,
        user_id: Option<&str>,
        state: &AppState,
    ) -> Result<ThreadSummary, ThreadError> {
        info!("Generating thread summary for root {}", thread_root_id);

        let thread_events = self.get_thread_events(thread_root_id, Some(50), None, state).await?;

        let latest_event = thread_events.last().cloned();
        let count = thread_events.len();

        // Get unique participants
        let mut participants: Vec<String> = thread_events
            .iter()
            .map(|e| e.sender.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        participants.sort();

        // Check if user participated
        let participated = if let Some(uid) = user_id {
            participants.contains(&uid.to_string())
        } else {
            false
        };

        Ok(ThreadSummary { latest_event, count, participated, participants })
    }

    pub async fn apply_thread_relation(
        &self,
        thread_root_id: &str,
        thread_event: &Value,
        state: &AppState,
    ) -> Result<(), ThreadError> {
        info!("Applying thread relation for root {}", thread_root_id);

        let thread_event_id =
            thread_event["event_id"].as_str().ok_or(ThreadError::InvalidThreadRoot)?;
        let room_id = thread_event["room_id"].as_str().ok_or(ThreadError::DifferentRoom)?;
        let sender = thread_event["sender"].as_str().ok_or(ThreadError::InvalidThreadRoot)?;

        // Store thread relation
        state
            .thread_repository
            .create_thread_relation(thread_event_id, thread_root_id, room_id, sender)
            .await
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?;

        // Update thread metadata
        self.update_thread_metadata(thread_root_id, state).await?;

        info!("Successfully applied thread relation for root {}", thread_root_id);
        Ok(())
    }

    pub async fn get_thread_participants(
        &self,
        thread_root_id: &str,
        state: &AppState,
    ) -> Result<Vec<String>, ThreadError> {
        state
            .thread_repository
            .get_thread_participants(thread_root_id)
            .await
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))
    }

    async fn validate_thread_depth(
        &self,
        thread_root_id: &str,
        state: &AppState,
    ) -> Result<(), ThreadError> {
        // Count current thread depth
        let count = state
            .thread_repository
            .get_thread_count(thread_root_id)
            .await
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?;

        const MAX_THREAD_DEPTH: u64 = 1000; // Reasonable limit
        if count >= MAX_THREAD_DEPTH {
            warn!("Thread depth {} exceeds maximum {}", count, MAX_THREAD_DEPTH);
            return Err(ThreadError::ExcessiveDepth);
        }

        Ok(())
    }

    async fn get_event(&self, event_id: &str, state: &AppState) -> Result<Event, ThreadError> {
        match state
            .thread_repository
            .get_event(event_id)
            .await
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?
        {
            Some(event) => Ok(event),
            None => Err(ThreadError::ThreadRootNotFound),
        }
    }

    async fn update_thread_metadata(
        &self,
        thread_root_id: &str,
        state: &AppState,
    ) -> Result<(), ThreadError> {
        let summary = self.generate_thread_summary(thread_root_id, None, state).await?;

        state
            .thread_repository
            .update_thread_summary(thread_root_id, &summary)
            .await
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?;

        Ok(())
    }
}

impl Default for ThreadManager {
    fn default() -> Self {
        Self::new()
    }
}
