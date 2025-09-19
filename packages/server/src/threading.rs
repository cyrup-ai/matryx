use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::state::AppState;

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

#[derive(Serialize, Deserialize)]
pub struct Event {
    pub event_id: String,
    pub room_id: String,
    pub sender: String,
    pub content: Value,
    pub origin_server_ts: u64,
    #[serde(rename = "type")]
    pub event_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct ThreadSummary {
    pub latest_event: Option<Event>,
    pub count: usize,
    pub participated: bool,
    pub participants: Vec<String>,
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
        let thread_room_id = thread_event["room_id"].as_str()
            .ok_or(ThreadError::DifferentRoom)?;
            
        if root_event.room_id != thread_room_id {
            warn!("Thread event room {} differs from root room {}", 
                  thread_room_id, root_event.room_id);
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
        let limit = limit.unwrap_or(50).min(100); // Cap at 100 events
        
        let query = "
            SELECT e.* FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $thread_root_id 
            AND r.rel_type = 'm.thread'
            ORDER BY e.origin_server_ts ASC
            LIMIT $limit
        ";
        
        let mut result = state.db
            .query(query)
            .bind(("thread_root_id", thread_root_id))
            .bind(("limit", limit))
            .await
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?;

        let events: Vec<Value> = result.take(0)
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?;

        let mut thread_events = Vec::new();
        for event_data in events {
            if let Ok(event) = self.value_to_event(event_data) {
                thread_events.push(event);
            }
        }

        Ok(thread_events)
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

        Ok(ThreadSummary {
            latest_event,
            count,
            participated,
            participants,
        })
    }

    pub async fn apply_thread_relation(
        &self,
        thread_root_id: &str,
        thread_event: &Value,
        state: &AppState,
    ) -> Result<(), ThreadError> {
        info!("Applying thread relation for root {}", thread_root_id);

        let thread_event_id = thread_event["event_id"].as_str()
            .ok_or(ThreadError::InvalidThreadRoot)?;
        let room_id = thread_event["room_id"].as_str()
            .ok_or(ThreadError::DifferentRoom)?;
        let sender = thread_event["sender"].as_str()
            .ok_or(ThreadError::InvalidThreadRoot)?;

        // Store thread relation
        let query = "
            CREATE event_relations SET
                event_id = $thread_event_id,
                relates_to_event_id = $thread_root_id,
                rel_type = 'm.thread',
                room_id = $room_id,
                sender = $sender,
                created_at = time::now()
        ";
        
        state.db
            .query(query)
            .bind(("thread_event_id", thread_event_id))
            .bind(("thread_root_id", thread_root_id))
            .bind(("room_id", room_id))
            .bind(("sender", sender))
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
        let query = "
            SELECT DISTINCT sender FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $thread_root_id 
            AND r.rel_type = 'm.thread'
            ORDER BY sender
        ";
        
        let mut result = state.db
            .query(query)
            .bind(("thread_root_id", thread_root_id))
            .await
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?;

        let participants: Vec<Value> = result.take(0)
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?;

        let participant_list: Vec<String> = participants
            .into_iter()
            .filter_map(|p| p.get("sender").and_then(|s| s.as_str()).map(|s| s.to_string()))
            .collect();

        Ok(participant_list)
    }

    async fn validate_thread_depth(
        &self,
        thread_root_id: &str,
        state: &AppState,
    ) -> Result<(), ThreadError> {
        // Count current thread depth
        let query = "
            SELECT COUNT(*) as count FROM room_timeline_events e
            JOIN event_relations r ON e.event_id = r.event_id
            WHERE r.relates_to_event_id = $thread_root_id 
            AND r.rel_type = 'm.thread'
        ";
        
        let mut result = state.db
            .query(query)
            .bind(("thread_root_id", thread_root_id))
            .await
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?;

        let counts: Vec<Value> = result.take(0)
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?;

        if let Some(count_data) = counts.first() {
            if let Some(count) = count_data.get("count").and_then(|c| c.as_u64()) {
                const MAX_THREAD_DEPTH: u64 = 1000; // Reasonable limit
                if count >= MAX_THREAD_DEPTH {
                    warn!("Thread depth {} exceeds maximum {}", count, MAX_THREAD_DEPTH);
                    return Err(ThreadError::ExcessiveDepth);
                }
            }
        }

        Ok(())
    }

    async fn get_event(
        &self,
        event_id: &str,
        state: &AppState,
    ) -> Result<Event, ThreadError> {
        let query = "
            SELECT event_id, room_id, sender, content, origin_server_ts, type
            FROM room_timeline_events 
            WHERE event_id = $event_id
            LIMIT 1
        ";
        
        let mut result = state.db
            .query(query)
            .bind(("event_id", event_id))
            .await
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?;

        let events: Vec<Value> = result.take(0)
            .map_err(|e| ThreadError::DatabaseError(e.to_string()))?;
        
        if let Some(event_data) = events.first() {
            self.value_to_event(event_data.clone())
        } else {
            Err(ThreadError::ThreadRootNotFound)
        }
    }

    fn value_to_event(&self, event_data: Value) -> Result<Event, ThreadError> {
        Ok(Event {
            event_id: event_data["event_id"].as_str().unwrap_or("").to_string(),
            room_id: event_data["room_id"].as_str().unwrap_or("").to_string(),
            sender: event_data["sender"].as_str().unwrap_or("").to_string(),
            content: event_data["content"].clone(),
            origin_server_ts: event_data["origin_server_ts"].as_u64().unwrap_or(0),
            event_type: event_data["type"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn update_thread_metadata(
        &self,
        thread_root_id: &str,
        state: &AppState,
    ) -> Result<(), ThreadError> {
        let summary = self.generate_thread_summary(thread_root_id, None, state).await?;
        
        let query = "
            UPDATE thread_metadata SET 
                count = $count,
                participants = $participants,
                latest_event_id = $latest_event_id,
                updated_at = time::now()
            WHERE thread_root_id = $thread_root_id
            ELSE CREATE thread_metadata SET
                id = rand::uuid(),
                thread_root_id = $thread_root_id,
                count = $count,
                participants = $participants,
                latest_event_id = $latest_event_id,
                created_at = time::now(),
                updated_at = time::now()
        ";
        
        let latest_event_id = summary.latest_event.as_ref().map(|e| e.event_id.as_str());
        
        state.db
            .query(query)
            .bind(("thread_root_id", thread_root_id))
            .bind(("count", summary.count as u64))
            .bind(("participants", serde_json::to_value(summary.participants)?))
            .bind(("latest_event_id", latest_event_id))
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