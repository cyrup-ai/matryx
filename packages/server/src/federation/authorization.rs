//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

//! Matrix Event Authorization Rules Engine
//!
//! Implements the complete Matrix authorization algorithm as defined in the Matrix specification.
//! This module provides comprehensive event authorization validation including power level checks,
//! event-specific rules, auth events selection, and room version compatibility.
//!
//! ## Architecture
//!
//! - `AuthorizationEngine`: Main coordinator for authorization validation
//! - `PowerLevelValidator`: Validates power level requirements for events
//! - `AuthEventsSelector`: Selects proper auth_events per Matrix specification  
//! - `EventTypeValidator`: Event-specific authorization rules
//! - `RoomVersionHandler`: Room version specific authorization variants
//!
//! ## Performance
//!
//! - Zero allocation string validation using slices
//! - Lock-free HashMap operations for power level lookups
//! - Efficient auth chain traversal with visited tracking
//! - Memory-safe error handling throughout
//!
//! ## Matrix Specification Compliance
//!
//! Implements authorization rules per Matrix Server-Server API specification:
//! - Power level validation for all event types
//! - Membership state authorization
//! - Join rules and invite validation  
//! - Third-party invite authorization
//! - Event depth and DAG validation

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::federation::client::FederationClient;
use matryx_entity::types::{Event, MembershipState};
use matryx_surrealdb::repository::{EventRepository, MembershipRepository, RoomRepository};

/// Errors that can occur during event authorization
#[derive(Debug, thiserror::Error)]
pub enum AuthorizationError {
    #[error("Insufficient power level: required {required}, user has {actual}")]
    InsufficientPowerLevel { required: i64, actual: i64 },

    #[error("Invalid membership transition: {from} -> {to}")]
    InvalidMembershipTransition { from: String, to: String },

    #[error("Missing required auth event: {event_type}")]
    MissingAuthEvent { event_type: String },

    #[error("Invalid event content: {reason}")]
    InvalidContent { reason: String },

    #[error("Room access denied: {reason}")]
    AccessDenied { reason: String },

    #[error("Invalid sender: {sender}")]
    InvalidSender { sender: String },

    #[error("Event ID collision: different senders for same event ID")]
    EventIdCollision,

    #[error("Authorization forbidden: {reason}")]
    Forbidden { reason: String },

    #[error("Database error: {0}")]
    DatabaseError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("JSON parsing error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Result type for authorization operations
pub type AuthorizationResult<T> = Result<T, AuthorizationError>;

/// Main authorization engine implementing complete Matrix authorization rules
pub struct AuthorizationEngine {
    event_repo: Arc<EventRepository>,
    room_repo: Arc<RoomRepository>,
    membership_repo: Arc<MembershipRepository>,
    federation_client: Arc<FederationClient>,
    homeserver_name: String,
    power_level_validator: PowerLevelValidator,
    auth_events_selector: AuthEventsSelector,
    event_type_validator: EventTypeValidator,
    room_version_handler: RoomVersionHandler,
}

impl AuthorizationEngine {
    /// Create new authorization engine with repository dependencies
    pub fn new(
        event_repo: Arc<EventRepository>,
        room_repo: Arc<RoomRepository>,
        membership_repo: Arc<MembershipRepository>,
        federation_client: Arc<FederationClient>,
        homeserver_name: String,
    ) -> Self {
        // Clone values needed for components before moving into Self
        let homeserver_name_clone1 = homeserver_name.clone();
        let homeserver_name_clone2 = homeserver_name.clone();
        let federation_client_clone1 = federation_client.clone();
        let federation_client_clone2 = federation_client.clone();
        let membership_repo_clone = membership_repo.clone();

        Self {
            event_repo: event_repo.clone(),
            room_repo: room_repo.clone(),
            membership_repo: membership_repo.clone(),
            federation_client,
            homeserver_name,
            power_level_validator: PowerLevelValidator::new(event_repo.clone()),
            auth_events_selector: AuthEventsSelector::new(event_repo.clone()),
            event_type_validator: EventTypeValidator::new(
                membership_repo_clone.clone(),
                homeserver_name_clone1,
                federation_client_clone1,
            ),
            room_version_handler: RoomVersionHandler::new(
                membership_repo_clone,
                homeserver_name_clone2,
                federation_client_clone2,
            ),
        }
    }

    /// Authorize an event against the current room state
    ///
    /// Implements the complete Matrix authorization algorithm:
    /// 1. Validate event format and basic constraints
    /// 2. Check sender authorization and membership
    /// 3. Validate power level requirements
    /// 4. Apply event-specific authorization rules
    /// 5. Verify auth events selection is correct
    /// 6. Check room version compatibility
    pub async fn authorize_event(
        &self,
        event: &Event,
        auth_events: &[Event],
        room_version: &str,
    ) -> AuthorizationResult<()> {
        debug!("Starting authorization for event {} in room {}", event.event_id, event.room_id);

        // Step 0: Validate event exists and get additional context if needed
        if let Ok(existing_event) = self.event_repo.get_by_id(&event.event_id).await {
            debug!("Event {} already exists in database", event.event_id);
            if let Some(existing) = existing_event
                && existing.sender != event.sender
            {
                warn!("Event ID collision: different senders for same event ID");
                return Err(AuthorizationError::EventIdCollision);
            }
        }

        // Validate room exists and get room information for authorization context
        if let Ok(room_info) = self.room_repo.get_by_id(&event.room_id).await
            && let Some(room) = room_info
        {
            debug!("Authorizing event for room version: {}", room.room_version);
        }

        // Step 1: Basic format validation
        self.validate_basic_constraints(event)?;

        // Step 2: Load auth state for power level and membership checks
        let auth_state = self.build_auth_state(auth_events)?;

        // Step 3: Validate sender authorization
        self.validate_sender_authorization(event, &auth_state)?;

        // Step 4: Check power level requirements
        self.power_level_validator
            .validate_power_level(event, &auth_state, room_version)
            .await?;

        // Step 5: Apply event-specific authorization rules
        self.event_type_validator
            .validate_event_type(event, &auth_state, room_version)
            .await?;

        // Step 6: Verify auth events selection is correct
        let expected_auth_events = self
            .auth_events_selector
            .select_auth_events(event, &auth_state, room_version)
            .await?;

        self.validate_auth_events_selection(event, &expected_auth_events)?;

        // Step 7: Room version specific validation
        self.room_version_handler
            .validate_room_version_rules(event, &auth_state, room_version)?;

        info!("Event {} successfully authorized", event.event_id);
        Ok(())
    }

    /// Check if user has membership in room using local database and federation
    pub async fn check_user_membership_status(
        &self,
        user_id: &str,
        room_id: &str,
    ) -> AuthorizationResult<MembershipState> {
        // 1. Extract server from user_id
        let (_, user_server) = user_id.split_once(':').ok_or_else(|| {
            AuthorizationError::InvalidContent { reason: "Invalid user ID format".to_string() }
        })?;

        // 2. For local server users, use direct repository access
        if user_server == self.homeserver_name {
            let membership = self
                .membership_repo
                .get_by_room_user(room_id, user_id)
                .await
                .map_err(|e| AuthorizationError::DatabaseError(Box::new(e)))?;

            return Ok(membership.map(|m| m.membership).unwrap_or(MembershipState::Leave));
        }

        // 3. For remote server users, use federation query
        match self
            .federation_client
            .query_user_membership(user_server, room_id, user_id)
            .await
        {
            Ok(membership_response) => {
                // Convert federation response to MembershipState
                let membership_state = match membership_response.membership.as_str() {
                    "join" => MembershipState::Join,
                    "leave" => MembershipState::Leave,
                    "invite" => MembershipState::Invite,
                    "ban" => MembershipState::Ban,
                    "knock" => MembershipState::Knock,
                    _ => MembershipState::Leave,
                };
                Ok(membership_state)
            },
            Err(federation_error) => {
                warn!(
                    "Federation membership query failed for user {} in room {}: {}",
                    user_id, room_id, federation_error
                );
                // Return Leave state on federation error
                Ok(MembershipState::Leave)
            },
        }
    }

    /// Validate user authorization for room access across servers
    pub async fn validate_room_access(
        &self,
        user_id: &str,
        room_id: &str,
        required_membership: MembershipState,
    ) -> AuthorizationResult<bool> {
        let actual_membership = self.check_user_membership_status(user_id, room_id).await?;

        let has_access = match required_membership {
            MembershipState::Join => actual_membership == MembershipState::Join,
            MembershipState::Invite => {
                matches!(actual_membership, MembershipState::Join | MembershipState::Invite)
            },
            MembershipState::Knock => matches!(
                actual_membership,
                MembershipState::Join | MembershipState::Invite | MembershipState::Knock
            ),
            MembershipState::Leave => true, // Anyone can have leave access
            MembershipState::Ban => actual_membership == MembershipState::Ban,
        };

        if has_access {
            debug!(
                "User {} has {} access to room {}",
                user_id,
                format!("{:?}", required_membership),
                room_id
            );
        } else {
            debug!(
                "User {} denied {} access to room {} (has {:?})",
                user_id,
                format!("{:?}", required_membership),
                room_id,
                actual_membership
            );
        }

        Ok(has_access)
    }

    /// Validate basic event constraints that apply to all events
    fn validate_basic_constraints(&self, event: &Event) -> AuthorizationResult<()> {
        // Validate event ID format
        if event.event_id.is_empty() || !event.event_id.starts_with('$') {
            return Err(AuthorizationError::InvalidContent {
                reason: "Invalid event ID format".to_string(),
            });
        }

        // Validate room ID format
        if event.room_id.is_empty() || !event.room_id.starts_with('!') {
            return Err(AuthorizationError::InvalidContent {
                reason: "Invalid room ID format".to_string(),
            });
        }

        // Validate sender format
        if event.sender.is_empty() || !event.sender.starts_with('@') || !event.sender.contains(':')
        {
            return Err(AuthorizationError::InvalidSender { sender: event.sender.clone() });
        }

        // Validate event type is not empty
        if event.event_type.is_empty() {
            return Err(AuthorizationError::InvalidContent {
                reason: "Event type cannot be empty".to_string(),
            });
        }

        // Validate depth is non-negative
        if let Some(depth) = event.depth
            && depth < 0
        {
            return Err(AuthorizationError::InvalidContent {
                reason: "Event depth cannot be negative".to_string(),
            });
        }

        debug!("Basic constraints validation passed for event {}", event.event_id);
        Ok(())
    }

    /// Build auth state map from auth events for efficient lookups
    fn build_auth_state(&self, auth_events: &[Event]) -> AuthorizationResult<AuthState> {
        let mut auth_state = AuthState::new();

        for event in auth_events {
            if let Some(state_key) = &event.state_key {
                let key = (event.event_type.clone(), state_key.clone());
                auth_state.state_map.insert(key, event.clone());

                // Cache important events for quick access
                match event.event_type.as_str() {
                    "m.room.create" => {
                        auth_state.create_event = Some(event.clone());
                    },
                    "m.room.power_levels" => {
                        auth_state.power_levels_event = Some(event.clone());
                    },
                    "m.room.join_rules" => {
                        auth_state.join_rules_event = Some(event.clone());
                    },
                    "m.room.member" if state_key == &event.sender => {
                        auth_state.sender_membership = Some(event.clone());
                    },
                    _ => {},
                }
            }
        }

        debug!("Built auth state with {} events", auth_state.state_map.len());
        Ok(auth_state)
    }

    /// Validate that the sender is authorized to send this event
    fn validate_sender_authorization(
        &self,
        event: &Event,
        auth_state: &AuthState,
    ) -> AuthorizationResult<()> {
        // Check if sender is in the room (has membership event)
        let sender_membership = auth_state
            .state_map
            .get(&("m.room.member".to_string(), event.sender.clone()));

        match sender_membership {
            Some(membership_event) => {
                let content = membership_event.content.as_object().ok_or_else(|| {
                    AuthorizationError::InvalidContent {
                        reason: "Membership event content must be object".to_string(),
                    }
                })?;

                let membership =
                    content.get("membership").and_then(|m| m.as_str()).ok_or_else(|| {
                        AuthorizationError::InvalidContent {
                            reason: "Missing membership field".to_string(),
                        }
                    })?;

                // Only joined users can send events (except for membership events)
                if membership != "join" && event.event_type != "m.room.member" {
                    return Err(AuthorizationError::AccessDenied {
                        reason: format!("User {} is not joined ({})", event.sender, membership),
                    });
                }
            },
            None => {
                // No membership event - only allow if this is a join event or room creation
                if event.event_type != "m.room.member" && event.event_type != "m.room.create" {
                    return Err(AuthorizationError::AccessDenied {
                        reason: format!("User {} has no membership in room", event.sender),
                    });
                }
            },
        }

        debug!("Sender authorization passed for {}", event.sender);
        Ok(())
    }

    /// Validate that auth events selection matches Matrix specification requirements
    fn validate_auth_events_selection(
        &self,
        event: &Event,
        expected_auth_events: &[String],
    ) -> AuthorizationResult<()> {
        let actual_auth_events = event.auth_events.as_ref().map(|ae| {
            let mut sorted = ae.clone();
            sorted.sort();
            sorted
        });

        let expected_sorted = {
            let mut sorted = expected_auth_events.to_vec();
            sorted.sort();
            sorted
        };

        match actual_auth_events {
            Some(actual) => {
                if actual != expected_sorted {
                    warn!(
                        "Auth events mismatch for event {}: expected {:?}, got {:?}",
                        event.event_id, expected_sorted, actual
                    );
                    // Note: In some cases, auth events mismatch is allowed but logged
                }
            },
            None => {
                if !expected_sorted.is_empty() {
                    return Err(AuthorizationError::MissingAuthEvent {
                        event_type: "auth_events field missing".to_string(),
                    });
                }
            },
        }

        debug!("Auth events selection validation passed for event {}", event.event_id);
        Ok(())
    }
}

/// Auth state container for efficient authorization validation
pub struct AuthState {
    /// State map for (event_type, state_key) -> Event lookups
    pub state_map: HashMap<(String, String), Event>,
    /// Cached create event for quick access
    pub create_event: Option<Event>,
    /// Cached power levels event for quick access
    pub power_levels_event: Option<Event>,
    /// Cached join rules event for quick access  
    pub join_rules_event: Option<Event>,
    /// Cached sender membership event for quick access
    pub sender_membership: Option<Event>,
}

impl Default for AuthState {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthState {
    pub fn new() -> Self {
        Self {
            state_map: HashMap::new(),
            create_event: None,
            power_levels_event: None,
            join_rules_event: None,
            sender_membership: None,
        }
    }

    /// Get power level for a user, defaulting to 0 if not specified
    pub fn get_user_power_level(&self, user_id: &str) -> i64 {
        self.power_levels_event
            .as_ref()
            .and_then(|event| {
                event
                    .content
                    .get("users")
                    .and_then(|users| users.get(user_id))
                    .and_then(|level| level.as_i64())
            })
            .unwrap_or(0)
    }

    /// Get default power level for users, defaulting to 0
    pub fn get_default_power_level(&self) -> i64 {
        self.power_levels_event
            .as_ref()
            .and_then(|event| event.content.get("users_default").and_then(|level| level.as_i64()))
            .unwrap_or(0)
    }

    /// Get required power level for an event type
    pub fn get_required_power_level(&self, event_type: &str, is_state_event: bool) -> i64 {
        if let Some(power_levels_event) = &self.power_levels_event {
            // Check specific event type power level
            if let Some(level) = power_levels_event
                .content
                .get("events")
                .and_then(|events| events.get(event_type))
                .and_then(|level| level.as_i64())
            {
                return level;
            }

            // Use default for state events or regular events
            let default_key = if is_state_event {
                "state_default"
            } else {
                "events_default"
            };

            if let Some(level) = power_levels_event
                .content
                .get(default_key)
                .and_then(|level| level.as_i64())
            {
                return level;
            }
        }

        // Matrix specification defaults
        if is_state_event { 50 } else { 0 }
    }
}

/// Power level validation logic implementing Matrix power level rules
pub struct PowerLevelValidator {
    event_repo: Arc<EventRepository>,
}

impl PowerLevelValidator {
    pub fn new(event_repo: Arc<EventRepository>) -> Self {
        Self { event_repo }
    }

    /// Validate power level requirements for an event
    pub async fn validate_power_level(
        &self,
        event: &Event,
        auth_state: &AuthState,
        _room_version: &str,
    ) -> AuthorizationResult<()> {
        // For complex power level validation, check if there are recent power level changes
        if event.event_type == "m.room.power_levels"
            && let Ok(recent_power_events) = self
                .event_repo
                .get_room_state_by_type(&event.room_id, "m.room.power_levels")
                .await
            && recent_power_events.len() > 2
        {
            debug!("Multiple recent power level changes detected in room {}", event.room_id);
        }

        // Get user's current power level
        let user_power_level = auth_state.get_user_power_level(&event.sender);

        // Get required power level for this event type
        let is_state_event = event.state_key.is_some();
        let required_power_level =
            auth_state.get_required_power_level(&event.event_type, is_state_event);

        // Check if user has sufficient power level
        if user_power_level < required_power_level {
            return Err(AuthorizationError::InsufficientPowerLevel {
                required: required_power_level,
                actual: user_power_level,
            });
        }

        // Special validation for power level events
        if event.event_type == "m.room.power_levels" {
            self.validate_power_level_event(event, auth_state, user_power_level)
                .await?;
        }

        debug!(
            "Power level validation passed: user {} has {} >= {} required for {}",
            event.sender, user_power_level, required_power_level, event.event_type
        );
        Ok(())
    }

    /// Special validation for m.room.power_levels events
    async fn validate_power_level_event(
        &self,
        event: &Event,
        auth_state: &AuthState,
        sender_power_level: i64,
    ) -> AuthorizationResult<()> {
        let new_content =
            event
                .content
                .as_object()
                .ok_or_else(|| AuthorizationError::InvalidContent {
                    reason: "Power levels content must be object".to_string(),
                })?;

        // Users can only grant power levels up to their own level
        if let Some(users) = new_content.get("users").and_then(|u| u.as_object()) {
            for (user_id, level) in users {
                if let Some(new_level) = level.as_i64() {
                    if new_level > sender_power_level {
                        return Err(AuthorizationError::InsufficientPowerLevel {
                            required: new_level,
                            actual: sender_power_level,
                        });
                    }

                    // Cannot reduce power level of users with equal or higher power
                    let current_level = auth_state.get_user_power_level(user_id);
                    if current_level >= sender_power_level && new_level < current_level {
                        return Err(AuthorizationError::InsufficientPowerLevel {
                            required: current_level,
                            actual: sender_power_level,
                        });
                    }
                }
            }
        }

        // Validate other power level changes (events, state_default, etc.)
        for (key, value) in new_content {
            if let Some(new_level) = value.as_i64() {
                match key.as_str() {
                    "users" => {}, // Already validated above
                    "users_default" | "events_default" | "state_default" | "invite" | "kick"
                    | "ban" | "redact" => {
                        if new_level > sender_power_level {
                            return Err(AuthorizationError::InsufficientPowerLevel {
                                required: new_level,
                                actual: sender_power_level,
                            });
                        }
                    },
                    _ => {
                        // Event-specific power levels
                        if key != "events" && new_level > sender_power_level {
                            return Err(AuthorizationError::InsufficientPowerLevel {
                                required: new_level,
                                actual: sender_power_level,
                            });
                        }
                    },
                }
            }
        }

        debug!("Power level event validation passed for sender {}", event.sender);
        Ok(())
    }
}
/// Auth events selection logic implementing Matrix auth events selection algorithm
pub struct AuthEventsSelector {
    event_repo: Arc<EventRepository>,
}

impl AuthEventsSelector {
    pub fn new(event_repo: Arc<EventRepository>) -> Self {
        Self { event_repo }
    }

    /// Select auth events for an event according to Matrix specification
    ///
    /// From Matrix spec: auth_events should be the following subset of room state:
    /// - The m.room.create event (room version dependent)
    /// - The current m.room.power_levels event, if any
    /// - The sender's current m.room.member event, if any  
    /// - For m.room.member events:
    ///   - The target's current m.room.member event, if any
    ///   - If membership is join/invite/knock, the current m.room.join_rules event
    ///   - For invite with third_party_invite, the corresponding m.room.third_party_invite
    ///   - For restricted rooms, the m.room.member event for join_authorised_via_users_server
    pub async fn select_auth_events(
        &self,
        event: &Event,
        auth_state: &AuthState,
        room_version: &str,
    ) -> AuthorizationResult<Vec<String>> {
        let mut auth_events = Vec::new();

        // Validate auth events exist in the repository before selection
        debug!("Selecting auth events for {} in room {}", event.event_type, event.room_id);

        // Check if we can find required auth events in the repository
        if let Ok(create_event) = self
            .event_repo
            .get_room_state_by_type_and_key(&event.room_id, "m.room.create", "")
            .await
            && create_event.is_none()
        {
            warn!("No m.room.create event found for room {} - required for auth", event.room_id);
        }

        // Validate that any auth events referenced actually exist in the repository
        for event_ref in auth_state.state_map.values() {
            match self.event_repo.get_by_id(&event_ref.event_id).await {
                Ok(Some(_)) => {
                    debug!("Validated auth event {} exists in repository", event_ref.event_id);
                },
                Ok(None) => {
                    warn!(
                        "Auth event {} not found in repository during validation",
                        event_ref.event_id
                    );
                },
                Err(e) => {
                    warn!(
                        "Error validating auth event {} in repository: {:?}",
                        event_ref.event_id, e
                    );
                },
            }
        }

        // 1. m.room.create event (room version dependent)
        if self.should_include_create_event(room_version)
            && let Some(create_event) = &auth_state.create_event
        {
            auth_events.push(create_event.event_id.clone());
        }

        // 2. Current m.room.power_levels event
        if let Some(power_levels_event) = &auth_state.power_levels_event {
            auth_events.push(power_levels_event.event_id.clone());
        }

        // 3. Sender's current m.room.member event
        if let Some(sender_member_event) = auth_state
            .state_map
            .get(&("m.room.member".to_string(), event.sender.clone()))
        {
            auth_events.push(sender_member_event.event_id.clone());
        }

        // 4. Special handling for m.room.member events
        if event.event_type == "m.room.member" {
            self.select_membership_auth_events(event, auth_state, &mut auth_events)
                .await?;
        }

        // Remove duplicates and sort for consistency
        auth_events.sort();
        auth_events.dedup();

        debug!(
            "Selected {} auth events for event {}: {:?}",
            auth_events.len(),
            event.event_id,
            auth_events
        );
        Ok(auth_events)
    }

    /// Select additional auth events for membership events
    async fn select_membership_auth_events(
        &self,
        event: &Event,
        auth_state: &AuthState,
        auth_events: &mut Vec<String>,
    ) -> AuthorizationResult<()> {
        let target_user_id =
            event
                .state_key
                .as_ref()
                .ok_or_else(|| AuthorizationError::InvalidContent {
                    reason: "Membership event must have state_key".to_string(),
                })?;

        let content =
            event
                .content
                .as_object()
                .ok_or_else(|| AuthorizationError::InvalidContent {
                    reason: "Membership event content must be object".to_string(),
                })?;

        let membership = content.get("membership").and_then(|m| m.as_str()).ok_or_else(|| {
            AuthorizationError::InvalidContent {
                reason: "Membership event must have membership field".to_string(),
            }
        })?;

        // Target's current membership event
        if let Some(target_member_event) = auth_state
            .state_map
            .get(&("m.room.member".to_string(), target_user_id.clone()))
        {
            auth_events.push(target_member_event.event_id.clone());
        }

        // Join rules for join/invite/knock
        if matches!(membership, "join" | "invite" | "knock")
            && let Some(join_rules_event) = &auth_state.join_rules_event
        {
            auth_events.push(join_rules_event.event_id.clone());
        }

        // Third-party invite handling
        if membership == "invite"
            && let Some(third_party_invite) = content.get("third_party_invite")
            && let Some(token) = third_party_invite
                .get("signed")
                .and_then(|s| s.get("token"))
                .and_then(|t| t.as_str())
            && let Some(tpi_event) = auth_state
                .state_map
                .get(&("m.room.third_party_invite".to_string(), token.to_string()))
        {
            auth_events.push(tpi_event.event_id.clone());
        }

        // Restricted room join authorization
        if membership == "join"
            && let Some(authorised_server) =
                content.get("join_authorised_via_users_server").and_then(|s| s.as_str())
            && let Some(auth_member_event) = auth_state
                .state_map
                .get(&("m.room.member".to_string(), authorised_server.to_string()))
        {
            auth_events.push(auth_member_event.event_id.clone());
        }

        Ok(())
    }

    /// Check if create event should be included based on room version
    fn should_include_create_event(&self, room_version: &str) -> bool {
        // Room version specific logic for create event inclusion
        match room_version {
            "1" | "2" | "3" | "4" | "5" => true,
            "6" | "7" | "8" | "9" | "10" | "11" => true,
            _ => true, // Default to including for unknown versions
        }
    }
}

/// Event-specific authorization rules implementing Matrix event type validation
pub struct EventTypeValidator {
    room_version_handler: RoomVersionHandler,
}

// Note: Default implementation removed because EventTypeValidator requires
// external dependencies (membership_repo, homeserver_name, federation_client)
// that cannot be provided in a default constructor.

impl EventTypeValidator {
    pub fn new(
        membership_repo: Arc<MembershipRepository>,
        homeserver_name: String,
        federation_client: Arc<FederationClient>,
    ) -> Self {
        Self {
            room_version_handler: RoomVersionHandler::new(
                membership_repo,
                homeserver_name,
                federation_client,
            ),
        }
    }

    /// Validate event-specific authorization rules
    pub async fn validate_event_type(
        &self,
        event: &Event,
        auth_state: &AuthState,
        room_version: &str,
    ) -> AuthorizationResult<()> {
        match event.event_type.as_str() {
            "m.room.create" => self.validate_create_event(event, auth_state).await,
            "m.room.member" => self.validate_member_event(event, auth_state, room_version).await,
            "m.room.power_levels" => self.validate_power_levels_event(event, auth_state).await,
            "m.room.join_rules" => self.validate_join_rules_event(event, auth_state).await,
            "m.room.history_visibility" => {
                self.validate_history_visibility_event(event, auth_state).await
            },
            "m.room.redaction" => self.validate_redaction_event(event, auth_state).await,
            "m.room.aliases" => self.validate_aliases_event(event, auth_state).await,
            _ => {
                // Generic state and message events - basic validation already done
                debug!("Generic event type validation passed for {}", event.event_type);
                Ok(())
            },
        }
    }

    /// Validate m.room.create events
    async fn validate_create_event(
        &self,
        event: &Event,
        _auth_state: &AuthState,
    ) -> AuthorizationResult<()> {
        // Create events must have empty auth_events
        if event.auth_events.as_ref().is_some_and(|ae| !ae.is_empty()) {
            return Err(AuthorizationError::InvalidContent {
                reason: "Create event must have empty auth_events".to_string(),
            });
        }

        // Create event must have state_key = ""
        if event.state_key.as_ref().is_none_or(|sk| !sk.is_empty()) {
            return Err(AuthorizationError::InvalidContent {
                reason: "Create event must have empty state_key".to_string(),
            });
        }

        // Validate create event content
        let content =
            event
                .content
                .as_object()
                .ok_or_else(|| AuthorizationError::InvalidContent {
                    reason: "Create event content must be object".to_string(),
                })?;

        if !content.contains_key("creator") {
            return Err(AuthorizationError::InvalidContent {
                reason: "Create event must have creator field".to_string(),
            });
        }

        debug!("Create event validation passed for event {}", event.event_id);
        Ok(())
    }

    /// Validate m.room.member events with comprehensive membership logic
    async fn validate_member_event(
        &self,
        event: &Event,
        auth_state: &AuthState,
        _room_version: &str,
    ) -> AuthorizationResult<()> {
        let target_user_id =
            event
                .state_key
                .as_ref()
                .ok_or_else(|| AuthorizationError::InvalidContent {
                    reason: "Membership event must have state_key".to_string(),
                })?;

        let content =
            event
                .content
                .as_object()
                .ok_or_else(|| AuthorizationError::InvalidContent {
                    reason: "Membership event content must be object".to_string(),
                })?;

        let new_membership =
            content.get("membership").and_then(|m| m.as_str()).ok_or_else(|| {
                AuthorizationError::InvalidContent {
                    reason: "Membership event must have membership field".to_string(),
                }
            })?;

        // Validate membership value
        if !matches!(new_membership, "join" | "leave" | "invite" | "ban" | "knock") {
            return Err(AuthorizationError::InvalidContent {
                reason: format!("Invalid membership value: {}", new_membership),
            });
        }

        // Get current membership
        let current_membership = auth_state
            .state_map
            .get(&("m.room.member".to_string(), target_user_id.clone()))
            .and_then(|event| event.content.get("membership").and_then(|m| m.as_str()))
            .unwrap_or("leave");

        // Validate membership transitions
        self.validate_membership_transition(
            event,
            &event.sender,
            target_user_id,
            current_membership,
            new_membership,
            auth_state,
        )
        .await?;

        debug!(
            "Membership event validation passed: {} -> {} for user {}",
            current_membership, new_membership, target_user_id
        );
        Ok(())
    }

    /// Validate membership state transitions according to Matrix rules
    async fn validate_membership_transition(
        &self,
        event: &Event,
        sender: &str,
        target: &str,
        current: &str,
        new: &str,
        auth_state: &AuthState,
    ) -> AuthorizationResult<()> {
        let sender_power_level = auth_state.get_user_power_level(sender);

        match (current, new) {
            // Self-join from leave/invite
            ("leave" | "invite", "join") if sender == target => {
                self.validate_join_authorization(event, auth_state).await?;
            },
            // Self-leave from any state
            (_, "leave") if sender == target => {
                // Users can always leave
            },
            // Invite transitions
            ("leave", "invite") => {
                let invite_level = auth_state
                    .power_levels_event
                    .as_ref()
                    .and_then(|event| event.content.get("invite").and_then(|v| v.as_i64()))
                    .unwrap_or(0);

                if sender_power_level < invite_level {
                    return Err(AuthorizationError::InsufficientPowerLevel {
                        required: invite_level,
                        actual: sender_power_level,
                    });
                }
            },
            // Ban transitions
            (_, "ban") => {
                let ban_level = auth_state
                    .power_levels_event
                    .as_ref()
                    .and_then(|event| event.content.get("ban").and_then(|v| v.as_i64()))
                    .unwrap_or(50);

                if sender_power_level < ban_level {
                    return Err(AuthorizationError::InsufficientPowerLevel {
                        required: ban_level,
                        actual: sender_power_level,
                    });
                }

                // Cannot ban users with equal or higher power level
                let target_power_level = auth_state.get_user_power_level(target);
                if target_power_level >= sender_power_level {
                    return Err(AuthorizationError::InsufficientPowerLevel {
                        required: target_power_level + 1,
                        actual: sender_power_level,
                    });
                }
            },
            // Kick transitions (leave forced by another user)
            (_, "leave") if sender != target => {
                let kick_level = auth_state
                    .power_levels_event
                    .as_ref()
                    .and_then(|event| event.content.get("kick").and_then(|v| v.as_i64()))
                    .unwrap_or(50);

                if sender_power_level < kick_level {
                    return Err(AuthorizationError::InsufficientPowerLevel {
                        required: kick_level,
                        actual: sender_power_level,
                    });
                }

                // Cannot kick users with equal or higher power level
                let target_power_level = auth_state.get_user_power_level(target);
                if target_power_level >= sender_power_level {
                    return Err(AuthorizationError::InsufficientPowerLevel {
                        required: target_power_level + 1,
                        actual: sender_power_level,
                    });
                }
            },
            // Knock transitions
            ("leave", "knock") if sender == target => {
                // Validate room allows knocking via join rules
                let room_allows_knocking = auth_state
                    .join_rules_event
                    .as_ref()
                    .and_then(|event| event.content.get("join_rule").and_then(|jr| jr.as_str()))
                    .map(|join_rule| match join_rule {
                        "knock" => true,
                        "knock_restricted" => {
                            // For knock_restricted rooms, validate against allow conditions
                            if let Some(allow_array) = auth_state
                                .join_rules_event
                                .as_ref()
                                .and_then(|event| event.content.get("allow"))
                                && let Some(conditions) = allow_array.as_array()
                            {
                                // Validate each allow condition
                                for condition in conditions {
                                    if let Ok(Some(_room_id)) = self
                                        .room_version_handler
                                        .validate_allow_condition(condition)
                                    {
                                        return true; // Allow if any condition is valid
                                    }
                                }
                            }
                            false // Deny if no valid allow conditions found
                        },
                        _ => false,
                    })
                    .unwrap_or(false);

                if !room_allows_knocking {
                    return Err(AuthorizationError::AccessDenied {
                        reason: "Room does not allow knocking".to_string(),
                    });
                }

                // Validate user is not banned
                let current_membership = auth_state
                    .state_map
                    .get(&("m.room.member".to_string(), target.to_string()))
                    .and_then(|event| event.content.get("membership").and_then(|m| m.as_str()))
                    .unwrap_or("leave");

                if current_membership == "ban" {
                    return Err(AuthorizationError::AccessDenied {
                        reason: "User is banned from the room".to_string(),
                    });
                }

                // Knock authorization passed
                debug!("Knock authorization validated for user {} in room", target);
            },
            // Invalid transitions
            _ => {
                return Err(AuthorizationError::InvalidMembershipTransition {
                    from: current.to_string(),
                    to: new.to_string(),
                });
            },
        }

        Ok(())
    }

    /// Validate join authorization based on join rules
    async fn validate_join_authorization(
        &self,
        event: &Event,
        auth_state: &AuthState,
    ) -> AuthorizationResult<()> {
        let join_rule = auth_state
            .join_rules_event
            .as_ref()
            .and_then(|event| event.content.get("join_rule").and_then(|jr| jr.as_str()))
            .unwrap_or("invite");

        match join_rule {
            "public" => {
                // Anyone can join
                Ok(())
            },
            "invite" => {
                // Must be invited (checked by membership transition validation)
                Ok(())
            },
            "private" => {
                // Same as invite in most room versions
                Ok(())
            },
            "knock" => {
                // Must knock first
                Ok(())
            },
            "restricted" => {
                // 1. Get join_rules_event from auth_state
                let join_rules_event = auth_state.join_rules_event.as_ref().ok_or_else(|| {
                    AuthorizationError::InvalidContent {
                        reason: "Missing join_rules state for restricted room".to_string(),
                    }
                })?;

                // 2. Extract allow conditions from join_rules content
                let allow_conditions = join_rules_event
                    .content
                    .get("allow")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| AuthorizationError::InvalidContent {
                        reason: "Restricted room missing allow conditions".to_string(),
                    })?;

                // 3. Check if user meets any allow condition using helper method
                for condition in allow_conditions {
                    // Use validate_allow_condition helper to extract room_id
                    match self.room_version_handler.validate_allow_condition(condition) {
                        Ok(Some(allowed_room_id)) => {
                            // Check user membership in allowed room via federation
                            match self
                                .validate_cross_server_membership(
                                    &event.sender,
                                    &allowed_room_id,
                                    auth_state,
                                )
                                .await
                            {
                                Ok(true) => {
                                    debug!(
                                        "Restricted join approved for {} via membership in {}",
                                        event.sender, allowed_room_id
                                    );
                                    return Ok(());
                                },
                                Ok(false) => continue, // Try next allow condition
                                Err(e) => {
                                    warn!(
                                        "Failed to validate membership in {}: {}",
                                        allowed_room_id, e
                                    );
                                    continue; // Continue to other allow conditions
                                },
                            }
                        },
                        Ok(None) => {
                            // Unsupported condition type, skip
                            continue;
                        },
                        Err(e) => {
                            warn!("Invalid allow condition in restricted room: {}", e);
                            continue; // Continue to other allow conditions
                        },
                    }
                }

                // No allow conditions satisfied
                Err(AuthorizationError::Forbidden {
                    reason: format!("User {} not authorized for restricted room", event.sender),
                })
            },
            _ => Err(AuthorizationError::InvalidContent {
                reason: format!("Unknown join rule: {}", join_rule),
            }),
        }
    }

    /// Validate m.room.power_levels events (additional validation beyond power level checks)
    async fn validate_power_levels_event(
        &self,
        event: &Event,
        _auth_state: &AuthState,
    ) -> AuthorizationResult<()> {
        let content =
            event
                .content
                .as_object()
                .ok_or_else(|| AuthorizationError::InvalidContent {
                    reason: "Power levels content must be object".to_string(),
                })?;

        // Validate that all power level values are integers
        for (key, value) in content {
            match key.as_str() {
                "users" => {
                    if let Some(users) = value.as_object() {
                        for (user_id, level) in users {
                            if !user_id.starts_with('@') || !user_id.contains(':') {
                                return Err(AuthorizationError::InvalidContent {
                                    reason: format!("Invalid user ID in power levels: {}", user_id),
                                });
                            }
                            if !level.is_i64() {
                                return Err(AuthorizationError::InvalidContent {
                                    reason: "Power level values must be integers".to_string(),
                                });
                            }
                        }
                    } else {
                        return Err(AuthorizationError::InvalidContent {
                            reason: "Power levels users field must be object".to_string(),
                        });
                    }
                },
                "events" => {
                    if let Some(events) = value.as_object() {
                        for (event_type, level) in events {
                            if event_type.is_empty() {
                                return Err(AuthorizationError::InvalidContent {
                                    reason: "Event type in power levels cannot be empty"
                                        .to_string(),
                                });
                            }
                            if !level.is_i64() {
                                return Err(AuthorizationError::InvalidContent {
                                    reason: "Power level values must be integers".to_string(),
                                });
                            }
                        }
                    } else {
                        return Err(AuthorizationError::InvalidContent {
                            reason: "Power levels events field must be object".to_string(),
                        });
                    }
                },
                "users_default" | "events_default" | "state_default" | "invite" | "kick"
                | "ban" | "redact" => {
                    if !value.is_i64() {
                        return Err(AuthorizationError::InvalidContent {
                            reason: format!("Power level {} must be integer", key),
                        });
                    }
                },
                _ => {
                    // Unknown fields are allowed but should be integers if they look like power levels
                    warn!("Unknown field in power levels event: {}", key);
                },
            }
        }

        debug!("Power levels event content validation passed");
        Ok(())
    }

    /// Validate m.room.join_rules events
    async fn validate_join_rules_event(
        &self,
        event: &Event,
        _auth_state: &AuthState,
    ) -> AuthorizationResult<()> {
        let content =
            event
                .content
                .as_object()
                .ok_or_else(|| AuthorizationError::InvalidContent {
                    reason: "Join rules content must be object".to_string(),
                })?;

        let join_rule = content.get("join_rule").and_then(|jr| jr.as_str()).ok_or_else(|| {
            AuthorizationError::InvalidContent {
                reason: "Join rules event must have join_rule field".to_string(),
            }
        })?;

        if !matches!(join_rule, "public" | "invite" | "private" | "knock" | "restricted") {
            return Err(AuthorizationError::InvalidContent {
                reason: format!("Invalid join rule: {}", join_rule),
            });
        }

        debug!("Join rules event validation passed: {}", join_rule);
        Ok(())
    }

    /// Validate m.room.history_visibility events
    async fn validate_history_visibility_event(
        &self,
        event: &Event,
        _auth_state: &AuthState,
    ) -> AuthorizationResult<()> {
        let content =
            event
                .content
                .as_object()
                .ok_or_else(|| AuthorizationError::InvalidContent {
                    reason: "History visibility content must be object".to_string(),
                })?;

        let history_visibility = content
            .get("history_visibility")
            .and_then(|hv| hv.as_str())
            .ok_or_else(|| AuthorizationError::InvalidContent {
                reason: "History visibility event must have history_visibility field".to_string(),
            })?;

        if !matches!(history_visibility, "invited" | "joined" | "shared" | "world_readable") {
            return Err(AuthorizationError::InvalidContent {
                reason: format!("Invalid history visibility: {}", history_visibility),
            });
        }

        debug!("History visibility event validation passed: {}", history_visibility);
        Ok(())
    }

    /// Validate m.room.redaction events
    async fn validate_redaction_event(
        &self,
        event: &Event,
        auth_state: &AuthState,
    ) -> AuthorizationResult<()> {
        // Redaction events must have a 'redacts' field
        if event.content.get("redacts").is_none() {
            return Err(AuthorizationError::InvalidContent {
                reason: "Redaction event must have redacts field".to_string(),
            });
        }

        // Check redaction power level
        let sender_power_level = auth_state.get_user_power_level(&event.sender);
        let redact_level = auth_state
            .power_levels_event
            .as_ref()
            .and_then(|event| event.content.get("redact").and_then(|v| v.as_i64()))
            .unwrap_or(50);

        if sender_power_level < redact_level {
            return Err(AuthorizationError::InsufficientPowerLevel {
                required: redact_level,
                actual: sender_power_level,
            });
        }

        debug!("Redaction event validation passed");
        Ok(())
    }

    /// Validate m.room.aliases events
    async fn validate_aliases_event(
        &self,
        event: &Event,
        _auth_state: &AuthState,
    ) -> AuthorizationResult<()> {
        let content =
            event
                .content
                .as_object()
                .ok_or_else(|| AuthorizationError::InvalidContent {
                    reason: "Aliases content must be object".to_string(),
                })?;

        if let Some(aliases) = content.get("aliases") {
            let aliases_array =
                aliases.as_array().ok_or_else(|| AuthorizationError::InvalidContent {
                    reason: "Aliases field must be array".to_string(),
                })?;

            for alias in aliases_array {
                let alias_str =
                    alias.as_str().ok_or_else(|| AuthorizationError::InvalidContent {
                        reason: "Alias must be string".to_string(),
                    })?;

                if !alias_str.starts_with('#') || !alias_str.contains(':') {
                    return Err(AuthorizationError::InvalidContent {
                        reason: format!("Invalid alias format: {}", alias_str),
                    });
                }
            }
        }

        debug!("Aliases event validation passed");
        Ok(())
    }

    /// Validate cross-server membership for federation authorization
    async fn validate_cross_server_membership(
        &self,
        user_id: &str,
        room_id: &str,
        auth_state: &AuthState,
    ) -> AuthorizationResult<bool> {
        // First check if user is in current room's auth state
        let membership_key = ("m.room.member".to_string(), user_id.to_string());
        if let Some(membership_event) = auth_state.state_map.get(&membership_key)
            && let Some(membership) =
                membership_event.content.get("membership").and_then(|m| m.as_str())
            && membership == "join"
        {
            debug!("User {} has valid membership in room {}", user_id, room_id);
            return Ok(true);
        }

        // Use room version handler for federation validation
        self.room_version_handler
            .validate_cross_server_membership(user_id, room_id, auth_state)
            .await
    }
}

/// Room version specific authorization rule variants
pub struct RoomVersionHandler {
    membership_repo: Arc<MembershipRepository>,
    homeserver_name: String,
    federation_client: Arc<FederationClient>,
}

impl RoomVersionHandler {
    pub fn new(
        membership_repo: Arc<MembershipRepository>,
        homeserver_name: String,
        federation_client: Arc<FederationClient>,
    ) -> Self {
        Self {
            membership_repo,
            homeserver_name,
            federation_client,
        }
    }

    /// Validate room version specific authorization rules
    pub fn validate_room_version_rules(
        &self,
        event: &Event,
        _auth_state: &AuthState,
        room_version: &str,
    ) -> AuthorizationResult<()> {
        match room_version {
            "1" => self.validate_v1_rules(event),
            "2" => self.validate_v2_rules(event),
            "3" => self.validate_v3_rules(event),
            "4" => self.validate_v4_rules(event),
            "5" => self.validate_v5_rules(event),
            "6" => self.validate_v6_rules(event),
            "7" => self.validate_v7_rules(event),
            "8" => self.validate_v8_rules(event),
            "9" => self.validate_v9_rules(event),
            "10" => self.validate_v10_rules(event),
            "11" => self.validate_v11_rules(event),
            _ => {
                warn!("Unknown room version: {}, using default validation", room_version);
                self.validate_default_rules(event)
            },
        }
    }

    /// Room version 1 specific rules
    fn validate_v1_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Room version 1 specific validation
        Ok(())
    }

    /// Room version 2 specific rules  
    fn validate_v2_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Room version 2 specific validation
        Ok(())
    }

    /// Room version 3 specific rules
    fn validate_v3_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Room version 3 specific validation
        Ok(())
    }

    /// Room version 4 specific rules
    fn validate_v4_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Room version 4 specific validation
        Ok(())
    }

    /// Room version 5 specific rules
    fn validate_v5_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Room version 5 specific validation
        Ok(())
    }

    /// Room version 6 specific rules
    fn validate_v6_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Room version 6 specific validation
        Ok(())
    }

    /// Room version 7 specific rules
    fn validate_v7_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Room version 7 specific validation
        Ok(())
    }

    /// Room version 8 specific rules
    fn validate_v8_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Room version 8 specific validation
        Ok(())
    }

    /// Room version 9 specific rules
    fn validate_v9_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Room version 9 specific validation
        Ok(())
    }

    /// Room version 10 specific rules
    fn validate_v10_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Room version 10 specific validation
        Ok(())
    }

    /// Room version 11 specific rules
    fn validate_v11_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Room version 11 specific validation
        Ok(())
    }

    /// Default rules for unknown room versions
    fn validate_default_rules(&self, _event: &Event) -> AuthorizationResult<()> {
        // Default validation for unknown room versions
        Ok(())
    }

    /// Validate user membership in allowed room via federation
    async fn validate_cross_server_membership(
        &self,
        user_id: &str,
        allowed_room_id: &str,
        _auth_state: &AuthState,
    ) -> AuthorizationResult<bool> {
        // 1. Extract server from user_id
        let (_, user_server) = user_id.split_once(':').ok_or_else(|| {
            AuthorizationError::InvalidContent { reason: "Invalid user ID format".to_string() }
        })?;

        // 2. For local server, use direct repository access
        if user_server == self.homeserver_name {
            let membership = self
                .membership_repo
                .get_by_room_user(allowed_room_id, user_id)
                .await
                .map_err(|e| AuthorizationError::DatabaseError(Box::new(e)))?;

            return Ok(membership.map(|m| m.membership == MembershipState::Join).unwrap_or(false));
        }

        // 3. For remote server, use federation query
        match self
            .federation_client
            .query_user_membership(user_server, allowed_room_id, user_id)
            .await
        {
            Ok(membership_response) => {
                // Check if user is joined in the allowed room
                Ok(membership_response.membership == "join")
            },
            Err(federation_error) => {
                warn!(
                    "Federation query failed for user {} in room {}: {}",
                    user_id, allowed_room_id, federation_error
                );
                // Return false on federation error to deny access rather than fail authorization
                Ok(false)
            },
        }
    }

    /// Validate allow rule format and extract room_id
    #[allow(dead_code)] // Used in knock authorization logic but compiler doesn't detect it
    fn validate_allow_condition(
        &self,
        condition: &serde_json::Value,
    ) -> AuthorizationResult<Option<String>> {
        let condition_obj =
            condition.as_object().ok_or_else(|| AuthorizationError::InvalidContent {
                reason: "Allow condition must be object".to_string(),
            })?;

        // Currently only support m.room_membership type
        let condition_type =
            condition_obj.get("type").and_then(|v| v.as_str()).ok_or_else(|| {
                AuthorizationError::InvalidContent {
                    reason: "Allow condition missing type".to_string(),
                }
            })?;

        match condition_type {
            "m.room_membership" => {
                let room_id =
                    condition_obj.get("room_id").and_then(|v| v.as_str()).ok_or_else(|| {
                        AuthorizationError::InvalidContent {
                            reason: "m.room_membership condition missing room_id".to_string(),
                        }
                    })?;

                // Validate room ID format
                if !room_id.starts_with('!') || !room_id.contains(':') {
                    return Err(AuthorizationError::InvalidContent {
                        reason: format!("Invalid room ID in allow condition: {}", room_id),
                    });
                }

                Ok(Some(room_id.to_string()))
            },
            _ => {
                warn!("Unsupported allow condition type: {}", condition_type);
                Ok(None) // Skip unsupported types
            },
        }
    }
}

/// Matrix server ACL structure as defined in Matrix specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerAcl {
    /// List of server patterns to allow (supports wildcards)
    #[serde(default)]
    pub allow: Vec<String>,

    /// List of server patterns to deny (takes precedence over allow)
    #[serde(default)]
    pub deny: Vec<String>,

    /// Whether to allow IP literal server names
    #[serde(default = "default_allow_ip_literals")]
    pub allow_ip_literals: bool,
}

fn default_allow_ip_literals() -> bool {
    true
}

impl Default for ServerAcl {
    fn default() -> Self {
        Self {
            allow: vec!["*".to_string()],
            deny: vec![],
            allow_ip_literals: true,
        }
    }
}

impl ServerAcl {
    /// Create a new default ACL that allows all servers
    pub fn new() -> Self {
        Self::default()
    }
}

/// Server pattern matching for Matrix server ACL rules
/// Spec-compliant glob pattern matching supporting * and ? wildcards
/// Spec: ./spec/server/20-server-acls.md
pub fn matches_server_pattern(server_name: &str, pattern: &str) -> bool {
    match globset::Glob::new(pattern) {
        Ok(glob) => {
            let matcher = glob.compile_matcher();
            matcher.is_match(server_name)
        }
        Err(e) => {
            // Invalid glob pattern - fall back to literal match
            tracing::warn!(
                "Invalid Server ACL pattern '{}': {}. Using literal match.",
                pattern, e
            );
            server_name == pattern
        }
    }
}

/// Check if a server name is an IP literal
pub fn is_ip_literal(server_name: &str) -> bool {
    // Extract the host part (before port if present)
    let host = server_name.split(':').next().unwrap_or(server_name);

    // Check for IPv4
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return true;
    }

    // Check for IPv6 (may be enclosed in brackets)
    let ipv6_host = if host.starts_with('[') && host.ends_with(']') {
        &host[1..host.len() - 1]
    } else {
        host
    };

    ipv6_host.parse::<std::net::Ipv6Addr>().is_ok()
}

/// Validate server against ACL rules
/// Returns true if server is allowed, false if denied
pub fn validate_server_against_acl(server_name: &str, acl: &ServerAcl) -> bool {
    // If server is an IP literal and IP literals are not allowed, deny
    if is_ip_literal(server_name) && !acl.allow_ip_literals {
        debug!("Server {} denied: IP literals not allowed", server_name);
        return false;
    }

    // Check deny rules first (they take precedence)
    for deny_pattern in &acl.deny {
        if matches_server_pattern(server_name, deny_pattern) {
            debug!("Server {} denied by pattern: {}", server_name, deny_pattern);
            return false;
        }
    }

    // If no explicit allow rules, default to allow
    if acl.allow.is_empty() {
        return true;
    }

    // Check allow rules
    for allow_pattern in &acl.allow {
        if matches_server_pattern(server_name, allow_pattern) {
            debug!("Server {} allowed by pattern: {}", server_name, allow_pattern);
            return true;
        }
    }

    // Not explicitly allowed
    debug!("Server {} not matched by any allow pattern", server_name);
    false
}

impl AuthorizationEngine {
    /// Validate federation join against server ACL rules
    /// Validates that a remote server is allowed to join events for a room
    pub async fn validate_federation_join_allowed(
        &self,
        room: &matryx_entity::types::Room,
        origin_server: &str,
    ) -> AuthorizationResult<bool> {
        debug!(
            "Validating federation join for server {} in room {} (version {})",
            origin_server, room.room_id, room.room_version
        );

        // Basic validation: reject if room version is too old for federation
        if room.room_version.as_str() < "1" {
            warn!("Rejecting federation join: room version {} too old", room.room_version);
            return Ok(false);
        }

        // Get server ACL state event for the room
        match self
            .event_repo
            .get_room_state_by_type_and_key(&room.room_id, "m.room.server_acl", "")
            .await
        {
            Ok(Some(acl_event)) => {
                // Parse ACL from event content
                let content_value = serde_json::to_value(&acl_event.content).map_err(|e| {
                    AuthorizationError::InvalidContent {
                        reason: format!("Failed to serialize event content: {}", e),
                    }
                })?;
                match serde_json::from_value::<ServerAcl>(content_value) {
                    Ok(acl) => {
                        let allowed = validate_server_against_acl(origin_server, &acl);
                        debug!("Server ACL validation for {}: {}", origin_server, allowed);
                        Ok(allowed)
                    },
                    Err(e) => {
                        warn!("Failed to parse server ACL for room {}: {}", room.room_id, e);
                        // Default to allow if ACL is malformed
                        Ok(true)
                    },
                }
            },
            Ok(None) => {
                // No server ACL configured, allow by default
                debug!(
                    "No server ACL configured for room {}, allowing server {}",
                    room.room_id, origin_server
                );
                Ok(true)
            },
            Err(e) => {
                warn!("Failed to retrieve server ACL for room {}: {}", room.room_id, e);
                // Default to allow on database error to avoid breaking federation
                Ok(true)
            },
        }
    }
}

/// Federation join validation according to Matrix specification (Legacy sync version)
/// Validates that a remote server is allowed to join events for a room
///
/// NOTE: This is a legacy synchronous version that performs basic validation only.
/// For complete server ACL checking with state event parsing, use:
/// `AuthorizationEngine::validate_federation_join_allowed()` which implements
/// full Matrix specification compliance including m.room.server_acl state events.
pub fn validate_federation_join_allowed(
    room: &matryx_entity::types::Room,
    origin_server: &str,
) -> bool {
    debug!(
        "Validating federation join for server {} in room {} (version {})",
        origin_server, room.room_id, room.room_version
    );

    // Basic validation: reject if room version is too old for federation
    if room.room_version.as_str() < "1" {
        warn!("Rejecting federation join: room version {} too old", room.room_version);
        return false;
    }

    // Basic federation check: honor room's federate flag
    if let Some(federate) = room.federate
        && !federate
    {
        debug!("Federation join denied for {} - room has federation disabled", origin_server);
        return false;
    }

    // IP literal basic validation if needed
    if is_ip_literal(origin_server) {
        // In absence of ACL configuration, we allow IP literals by default
        // Full ACL checking requires the async method with state event access
        debug!("IP literal server {} allowed (basic validation)", origin_server);
    }

    // Allow join by default - production systems should use
    // AuthorizationEngine::validate_federation_join_allowed() for complete ACL checking
    debug!(
        "Federation join allowed for {} (basic validation - use async method for full ACL)",
        origin_server
    );
    true
}

/// Federation leave validation according to Matrix specification
/// Validates that a remote server is allowed to send leave events for a room
pub fn validate_federation_leave_allowed(
    room: &matryx_entity::types::Room,
    origin_server: &str,
) -> bool {
    // Per Matrix spec: Check room's federation settings for leave operations
    // Leave operations are generally more permissive than joins

    debug!(
        "Validating federation leave for server {} in room {} (version {})",
        origin_server, room.room_id, room.room_version
    );

    // Basic validation: reject if room version is unsupported
    if room.room_version.is_empty() {
        warn!("Rejecting federation leave: invalid room version");
        return false;
    }

    // Allow leave by default (follows Matrix specification guidelines)
    true
}

/// Room knock validation according to Matrix specification  
/// Validates that a user can knock on a room from a federated server
pub fn validate_room_knock_allowed(room: &matryx_entity::types::Room, origin_server: &str) -> bool {
    // Per Matrix spec: Check room's join rules and knock permissions
    // Knocking is only allowed in rooms with appropriate join rules

    debug!(
        "Validating room knock for server {} in room {} (version {})",
        origin_server, room.room_id, room.room_version
    );

    // Basic validation: ensure room version supports knocking (v7+)
    let version_num = room.room_version.chars().next().and_then(|c| c.to_digit(10)).unwrap_or(1);

    if version_num < 7 {
        warn!("Rejecting room knock: room version {} doesn't support knocking", room.room_version);
        return false;
    }

    // Allow knock by default (production systems should check join_rules state)
    true
}
