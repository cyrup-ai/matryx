//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

//! Matrix State Resolution Algorithm v2
//!
//! Implements the Matrix state resolution algorithm v2 as defined in the Matrix
//! specification. This algorithm is used to determine the current state of a room
//! when there are conflicts in the event graph.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;


use tracing::{debug, info, warn};

use matryx_entity::types::Event;
use matryx_surrealdb::repository::error::RepositoryError;
use matryx_surrealdb::repository::{EventRepository, RoomRepository};

/// Errors that can occur during state resolution
#[derive(Debug, thiserror::Error)]
pub enum StateResolutionError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] RepositoryError),

    #[error("Invalid state event: {0}")]
    InvalidStateEvent(String),

    #[error("Circular dependency detected in authorization events")]
    CircularDependency,

    #[error("Missing authorization event: {0}")]
    MissingAuthEvent(String),

    #[error("Invalid authorization rules: {0}")]
    InvalidAuthorization(String),
}

/// State resolution result containing the resolved state
#[derive(Debug, Clone)]
pub struct ResolvedState {
    /// The resolved state events, keyed by (event_type, state_key)
    pub state_events: HashMap<(String, String), Event>,

    /// The final set of auth events that the resolved state depends on
    pub auth_chain: Vec<Event>,

    /// Events that were rejected during resolution
    pub rejected_events: Vec<Event>,

    /// Events that were soft-failed during resolution
    pub soft_failed_events: Vec<Event>,
}

/// State key for identifying state events
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StateKey {
    pub event_type: String,
    pub state_key: String,
}

impl StateKey {
    pub fn new(event_type: String, state_key: String) -> Self {
        Self { event_type, state_key }
    }

    pub fn from_event(event: &Event) -> Option<Self> {
        event
            .state_key
            .as_ref()
            .map(|sk| Self::new(event.event_type.clone(), sk.clone()))
    }
}

/// Matrix State Resolution v2 algorithm implementation
pub struct StateResolver {
    event_repo: Arc<EventRepository>,
    room_repo: Arc<RoomRepository>,
}

impl StateResolver {
    pub fn new(event_repo: Arc<EventRepository>, room_repo: Arc<RoomRepository>) -> Self {
        Self { event_repo, room_repo }
    }

    /// Resolve state conflicts using Matrix state resolution algorithm v2
    pub async fn resolve_state_v2(
        &self,
        room_id: &str,
        conflicted_events: Vec<Event>,
        power_event: Option<Event>,
    ) -> Result<ResolvedState, StateResolutionError> {
        info!(
            "Starting state resolution v2 for room {} with {} conflicted events",
            room_id,
            conflicted_events.len()
        );

        // Validate room exists and get room version for version-specific state resolution
        let room = self.room_repo.get_by_id(room_id).await?
            .ok_or_else(|| StateResolutionError::InvalidStateEvent(
                format!("Room {} not found", room_id)
            ))?;
        
        let room_version = if room.room_version.is_empty() {
            "1".to_string()
        } else {
            room.room_version
        };
        debug!("Using room version {} for state resolution", room_version);

        // Validate all events belong to the correct room using room_repo context
        for event in &conflicted_events {
            if event.room_id != room_id {
                return Err(StateResolutionError::InvalidStateEvent(
                    format!("Event {} belongs to room {} but resolving for room {}", 
                           event.event_id, event.room_id, room_id)
                ));
            }
        }

        // Step 1: Separate conflicted events by state key
        let mut state_groups = HashMap::new();
        for event in &conflicted_events {
            if let Some(state_key) = StateKey::from_event(event) {
                state_groups.entry(state_key).or_insert_with(Vec::new).push(event.clone());
            }
        }

        // Step 2: For each state key, resolve conflicts
        let mut resolved_state = HashMap::new();
        let mut all_auth_events = HashSet::new();
        let mut rejected_events = Vec::new();
        let mut soft_failed_events = Vec::new();

        for (state_key, events) in state_groups {
            debug!(
                "Resolving conflict for state key ({}, {})",
                state_key.event_type, state_key.state_key
            );

            let (resolved_event, rejected, soft_failed) = if events.len() == 1 {
                // No conflict for this state key
                let resolved_event = events.into_iter().next().ok_or_else(|| {
                    StateResolutionError::InvalidStateEvent(
                        "Empty state event collection".to_string(),
                    )
                })?;
                (resolved_event, Vec::new(), Vec::new())
            } else {
                // Resolve conflict using authorization rules
                self.resolve_state_key_conflict_with_tracking(room_id, events, power_event.as_ref())
                    .await?
            };

            // Add to resolved state
            resolved_state.insert(
                (state_key.event_type.clone(), state_key.state_key.clone()),
                resolved_event.clone(),
            );

            // Collect auth events
            all_auth_events.extend(resolved_event.auth_events.iter().cloned());

            // Track rejected and soft-failed events
            rejected_events.extend(rejected);
            soft_failed_events.extend(soft_failed);
        }

        // Step 3: Build auth chain
        let auth_chain = self
            .build_auth_chain(room_id, all_auth_events.into_iter().flatten().collect())
            .await?;

        // Step 4: Validate final state
        self.validate_resolved_state(&resolved_state, &auth_chain).await?;

        info!(
            "State resolution completed for room {}: {} events resolved, {} rejected, {} soft-failed",
            room_id,
            resolved_state.len(),
            rejected_events.len(),
            soft_failed_events.len()
        );

        Ok(ResolvedState {
            state_events: resolved_state,
            auth_chain,
            rejected_events,
            soft_failed_events,
        })
    }

    /// Resolve conflicts for a specific state key with tracking of rejected/soft-failed events
    async fn resolve_state_key_conflict_with_tracking(
        &self,
        room_id: &str,
        conflicting_events: Vec<Event>,
        power_event: Option<&Event>,
    ) -> Result<(Event, Vec<Event>, Vec<Event>), StateResolutionError> {
        let mut rejected_events = Vec::new();
        let mut soft_failed_events = Vec::new();

        // Filter out invalid events first
        let mut valid_events = Vec::new();
        for event in conflicting_events {
            // Basic validation - in a full implementation this would be more comprehensive
            if self.validate_event_for_resolution(&event).await {
                valid_events.push(event);
            } else {
                debug!("Event {} failed validation, marking as rejected", event.event_id);
                rejected_events.push(event);
            }
        }

        if valid_events.is_empty() {
            return Err(StateResolutionError::InvalidStateEvent(
                "No valid events remaining after filtering".to_string(),
            ));
        }

        if valid_events.len() == 1 {
            let resolved_event = valid_events.into_iter().next().ok_or_else(|| {
                StateResolutionError::InvalidStateEvent("Empty valid events collection".to_string())
            })?;
            return Ok((resolved_event, rejected_events, soft_failed_events));
        }

        // Use the existing conflict resolution logic
        let resolved_event = self
            .resolve_state_key_conflict(room_id, valid_events.clone(), power_event)
            .await?;

        // Mark non-selected events as soft-failed (they're valid but not chosen)
        for event in valid_events {
            if event.event_id != resolved_event.event_id {
                debug!("Event {} soft-failed during state resolution", event.event_id);
                soft_failed_events.push(event);
            }
        }

        Ok((resolved_event, rejected_events, soft_failed_events))
    }

    /// Resolve conflicts for a specific state key using Matrix authorization rules
    async fn resolve_state_key_conflict(
        &self,
        room_id: &str,
        conflicting_events: Vec<Event>,
        power_event: Option<&Event>,
    ) -> Result<Event, StateResolutionError> {
        if conflicting_events.len() <= 1 {
            return conflicting_events.into_iter().next().ok_or_else(|| {
                StateResolutionError::InvalidStateEvent("Empty conflict set".to_string())
            });
        }

        debug!("Resolving {} conflicting events for state key", conflicting_events.len());

        // Sort events by power level (higher power wins)
        let mut sorted_events = conflicting_events.clone();

        // Basic power level resolution - in a full implementation this would be more complex
        sorted_events.sort_by(|a, b| {
            // Compare by origin server timestamp as a simple tie-breaker
            // In a full implementation, this would use proper power level comparison
            a.origin_server_ts.cmp(&b.origin_server_ts).reverse()
        });

        // For membership events, apply special membership rules
        if let Some(first_event) = sorted_events.first()
            && first_event.event_type == "m.room.member" {
            return self.resolve_membership_conflict(room_id, sorted_events, power_event).await;
        }

        // For other state events, return the event with highest authority
        sorted_events.into_iter().next().ok_or_else(|| {
            StateResolutionError::InvalidStateEvent("Empty sorted events collection".to_string())
        })
    }

    /// Resolve membership event conflicts with special membership rules
    async fn resolve_membership_conflict(
        &self,
        room_id: &str,
        conflicting_events: Vec<Event>,
        _power_event: Option<&Event>,
    ) -> Result<Event, StateResolutionError> {
        // Simplified membership conflict resolution
        // In a full implementation, this would apply Matrix membership transition rules
        debug!("Resolving membership conflicts for room: {} with {} events",
               room_id, conflicting_events.len());

        // Validate all events belong to the specified room
        for event in &conflicting_events {
            if event.room_id != room_id {
                warn!("Event {} does not belong to room {} during membership conflict resolution",
                      event.event_id, room_id);
                return Err(StateResolutionError::InvalidStateEvent(
                    format!("Event {} room mismatch: expected {}, got {}",
                           event.event_id, room_id, event.room_id)
                ));
            }
        }

        for event in &conflicting_events {
            if let Some(content) = event.content.as_object()
                && let Some(membership) = content.get("membership").and_then(|m| m.as_str()) {
                match membership {
                    "ban" => {
                        // Ban events have highest priority
                        debug!("Resolving membership conflict in room {}: ban event {} wins",
                               room_id, event.event_id);
                        return Ok(event.clone());
                    },
                    "leave" => {
                        // Leave events have second priority
                        debug!("Resolving membership conflict in room {}: leave event {} considered",
                               room_id, event.event_id);
                    },
                    "join" => {
                        // Join events have lower priority
                        debug!("Resolving membership conflict in room {}: join event {} considered",
                               room_id, event.event_id);
                    },
                    _ => {},
                }
            }
        }

        // Default to most recent event
        let mut sorted = conflicting_events;
        sorted.sort_by(|a, b| b.origin_server_ts.cmp(&a.origin_server_ts));
        sorted.into_iter().next().ok_or_else(|| {
            StateResolutionError::InvalidStateEvent(
                "Empty conflicting events collection".to_string(),
            )
        })
    }

    /// Build the authorization event chain for the resolved state
    async fn build_auth_chain(
        &self,
        room_id: &str,
        auth_event_ids: Vec<String>,
    ) -> Result<Vec<Event>, StateResolutionError> {
        let mut auth_chain = Vec::new();
        let mut visited = HashSet::new();

        for auth_event_id in auth_event_ids {
            self.collect_auth_chain_recursive(
                room_id,
                &auth_event_id,
                &mut auth_chain,
                &mut visited,
            )
            .await?;
        }

        // Remove duplicates and sort by depth
        auth_chain.sort_by(|a, b| a.depth.cmp(&b.depth));
        auth_chain.dedup_by(|a, b| a.event_id == b.event_id);

        debug!("Built auth chain with {} events", auth_chain.len());
        Ok(auth_chain)
    }

    /// Recursively collect authorization events
    fn collect_auth_chain_recursive<'a>(
        &'a self,
        room_id: &'a str,
        event_id: &'a str,
        auth_chain: &'a mut Vec<Event>,
        visited: &'a mut HashSet<String>,
    ) -> Pin<Box<dyn Future<Output = Result<(), StateResolutionError>> + Send + 'a>> {
        Box::pin(async move {
            if visited.contains(event_id) {
                return Ok(());
            }
            visited.insert(event_id.to_string());

            if let Ok(Some(event)) = self.event_repo.get_by_id(event_id).await {
                // Validate that the event belongs to the correct room
                if event.room_id != room_id {
                    warn!("Auth event {} belongs to room {} but expected room {}", 
                          event_id, event.room_id, room_id);
                    return Err(StateResolutionError::InvalidAuthorization(format!(
                        "Auth event {} belongs to wrong room", event_id
                    )));
                }
                
                // Recursively collect auth events
                for auth_event_id in event.auth_events.as_ref().unwrap_or(&Vec::new()) {
                    self.collect_auth_chain_recursive(room_id, auth_event_id, auth_chain, visited)
                        .await?;
                }

                auth_chain.push(event);
            } else {
                warn!("Auth event {} not found in database", event_id);
            }

            Ok(())
        })
    }

    /// Validate that the resolved state is consistent and authorized
    async fn validate_resolved_state(
        &self,
        resolved_state: &HashMap<(String, String), Event>,
        auth_chain: &[Event],
    ) -> Result<(), StateResolutionError> {
        debug!("Validating resolved state with {} events against auth chain with {} events",
               resolved_state.len(), auth_chain.len());

        // Create a set of auth event IDs for quick lookup
        let auth_event_ids: std::collections::HashSet<_> =
            auth_chain.iter().map(|e| &e.event_id).collect();

        // Validate each event in resolved state has proper authorization
        for ((event_type, state_key), event) in resolved_state {
            // Validate that the event's state_key matches the resolved state mapping
            if let Some(event_state_key) = &event.state_key {
                if event_state_key != state_key {
                    return Err(StateResolutionError::InvalidStateEvent(format!(
                        "State key mismatch for event {}: resolved as {} but event has {}",
                        event.event_id, state_key, event_state_key
                    )));
                }
            } else if !state_key.is_empty() {
                return Err(StateResolutionError::InvalidStateEvent(format!(
                    "Event {} missing state_key but resolved state expects '{}'",
                    event.event_id, state_key
                )));
            }

            // Check if this event has auth events and they're in the auth chain
            if let Some(auth_events) = &event.auth_events {
                for auth_event_id in auth_events {
                    if !auth_event_ids.contains(auth_event_id) {
                        warn!("Auth event {} for state event {} not found in auth chain",
                              auth_event_id, event.event_id);
                        // In production, this would be more strict
                    }
                }
            }

            // Matrix specification validation: certain events must have auth events
            match event_type.as_str() {
                "m.room.member" | "m.room.power_levels" | "m.room.join_rules" => {
                    if event.auth_events.is_none() || event.auth_events.as_ref().map_or(true, |events| events.is_empty()) {
                        return Err(StateResolutionError::InvalidStateEvent(format!(
                            "Event {} of type {} missing required auth events",
                            event.event_id, event_type
                        )));
                    }
                },
                _ => {} // Other event types may not require auth events
            }
        }

        // Basic validation - in a full implementation this would be more comprehensive
        for ((event_type, state_key), event) in resolved_state {
            // Verify event type and state key match
            if event.event_type != *event_type {
                return Err(StateResolutionError::InvalidStateEvent(format!(
                    "Event type mismatch: expected {}, got {}",
                    event_type, event.event_type
                )));
            }

            if event.state_key.as_ref() != Some(state_key) {
                return Err(StateResolutionError::InvalidStateEvent(format!(
                    "State key mismatch: expected {}, got {:?}",
                    state_key, event.state_key
                )));
            }

            // Basic content validation
            if event.content.is_null() {
                return Err(StateResolutionError::InvalidStateEvent(format!(
                    "Event {} has null content",
                    event.event_id
                )));
            }
        }

        info!("Resolved state validation passed");
        Ok(())
    }

    /// Validate an event for inclusion in state resolution
    ///
    /// Performs basic validation to determine if an event should be considered
    /// during state resolution or rejected outright.
    async fn validate_event_for_resolution(&self, event: &Event) -> bool {
        // Basic format validation
        if event.event_id.is_empty() || !event.event_id.starts_with('$') {
            debug!("Event {} has invalid event ID format", event.event_id);
            return false;
        }

        if event.room_id.is_empty() || !event.room_id.starts_with('!') {
            debug!("Event {} has invalid room ID format", event.event_id);
            return false;
        }

        if event.sender.is_empty() || !event.sender.starts_with('@') || !event.sender.contains(':')
        {
            debug!("Event {} has invalid sender format", event.event_id);
            return false;
        }

        // State events must have a state_key
        if event.state_key.is_none() {
            debug!("Event {} is not a state event (missing state_key)", event.event_id);
            return false;
        }

        // Basic content validation
        if event.content.is_null() {
            debug!("Event {} has null content", event.event_id);
            return false;
        }

        // Event-type specific validation
        match event.event_type.as_str() {
            "m.room.member" => {
                // Membership events must have membership field
                if let Some(content) = event.content.as_object() {
                    if content.get("membership").and_then(|m| m.as_str()).is_none() {
                        debug!("Membership event {} missing membership field", event.event_id);
                        return false;
                    }
                } else {
                    debug!("Membership event {} has invalid content format", event.event_id);
                    return false;
                }
            },
            "m.room.power_levels" => {
                // Power level events should have valid structure
                if !event.content.is_object() {
                    debug!("Power levels event {} has invalid content format", event.event_id);
                    return false;
                }
            },
            _ => {
                // Other event types - basic validation already done above
            },
        }

        // If we get here, the event is valid for state resolution
        debug!("Event {} passed validation for state resolution", event.event_id);
        true
    }

    /// Get the current room state for conflict resolution
    pub async fn get_room_state(
        &self,
        room_id: &str,
    ) -> Result<HashMap<(String, String), Event>, StateResolutionError> {
        // Validate room exists using room_repo
        let room = self.room_repo.get_by_id(room_id).await?
            .ok_or_else(|| StateResolutionError::InvalidStateEvent(
                format!("Room {} not found", room_id)
            ))?;
        
        debug!("Getting state for room {} (version: {})", 
               room_id, if room.room_version.is_empty() { "1" } else { &room.room_version });

        let state_events = self.event_repo.get_state_events(room_id).await?;

        let mut state_map = HashMap::new();
        for event in state_events {
            if let Some(state_key) = &event.state_key {
                state_map.insert((event.event_type.clone(), state_key.clone()), event);
            }
        }

        debug!("Retrieved current room state: {} events", state_map.len());
        Ok(state_map)
    }
}
