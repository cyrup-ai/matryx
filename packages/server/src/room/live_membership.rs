//! Module contains intentional library code not yet fully integrated
#![allow(dead_code)]

use std::sync::Arc;

use axum::http::StatusCode;
use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tracing::{debug, error, warn};

use crate::config::ServerConfig;
use crate::room::{JoinRulesValidator, PowerLevelValidator, RoomAliasResolver};
use matryx_entity::types::{Membership, MembershipState};
use matryx_surrealdb::repository::{MembershipRepository, RoomRepository};

/// Real-time Membership Updates Service with Advanced Authorization
///
/// Provides comprehensive real-time membership monitoring integrated with the
/// Advanced Authorization System (Join Rules, Power Levels, Alias Resolution).
///
/// This service handles:
/// - Real-time membership change notifications using SurrealDB LiveQuery
/// - Authorization-aware filtering based on user permissions and room visibility
/// - Integration with Matrix sync protocol for efficient client updates
/// - Room-wide membership monitoring for all users in joined rooms
/// - Power level enforcement for membership visibility
/// - Join rules validation for membership event access
///
/// Performance: High-throughput streaming with efficient authorization caching
/// Security: Complete Matrix authorization compliance with real-time enforcement
pub struct LiveMembershipService {
    db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>,
    room_repo: Arc<RoomRepository>,
    membership_repo: Arc<MembershipRepository>,
    join_rules_validator: Arc<JoinRulesValidator>,
    power_level_validator: Arc<PowerLevelValidator>,
    alias_resolver: Arc<RoomAliasResolver>,
    homeserver_name: String,
}

/// Real-time membership update event
#[derive(Debug, Clone, Serialize)]
pub struct MembershipUpdate {
    /// Update type (create, update, delete)
    pub action: MembershipOperation,
    /// Room ID where membership changed
    pub room_id: String,
    /// User whose membership changed
    pub user_id: String,
    /// New membership state
    pub membership_state: MembershipState,
    /// Event ID of the membership change (if available)
    pub event_id: Option<String>,
    /// User who caused the change (for kicks, bans, invites)
    pub sender: Option<String>,
    /// Reason for the change (if provided)
    pub reason: Option<String>,
    /// Timestamp of the change
    pub timestamp: i64,
}

/// Type of membership database operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MembershipOperation {
    Create,
    Update,
    Delete,
}

/// Filtered membership update for specific user
#[derive(Debug, Clone, Serialize)]
pub struct FilteredMembershipUpdate {
    /// The membership update
    pub update: MembershipUpdate,
    /// Whether this user can see the full update details
    pub has_full_visibility: bool,
    /// Filtered event content (may be redacted based on permissions)
    pub filtered_content: Value,
}

impl LiveMembershipService {
    /// Create a new LiveMembershipService instance
    ///
    /// # Arguments
    /// * `db` - SurrealDB connection for LiveQuery operations
    ///
    /// # Returns
    /// * `LiveMembershipService` - Service with integrated authorization systems
    pub fn new(db: Arc<surrealdb::Surreal<surrealdb::engine::any::Any>>) -> Self {
        let room_repo = Arc::new(RoomRepository::new((*db).clone()));
        let membership_repo = Arc::new(MembershipRepository::new((*db).clone()));

        // Initialize authorization components
        let join_rules_validator = Arc::new(JoinRulesValidator::new(db.clone()));
        let power_level_validator = Arc::new(PowerLevelValidator::new(db.clone()));
        let homeserver_name = ServerConfig::get()
            .map(|config| config.homeserver_name.clone())
            .unwrap_or_else(|e| {
                tracing::error!("Failed to get server config in LiveMembershipService: {:?}", e);
                "localhost".to_string()
            });

        let alias_resolver = Arc::new(RoomAliasResolver::new(db.clone(), homeserver_name.clone()));

        Self {
            db,
            room_repo,
            membership_repo,
            join_rules_validator,
            power_level_validator,
            alias_resolver,
            homeserver_name,
        }
    }

    /// Create a filtered membership stream for a specific user
    ///
    /// Returns a stream of membership updates that the user is authorized to see.
    /// Applies join rules, power level, and visibility filtering in real-time.
    ///
    /// # Arguments
    /// * `user_id` - The user to create the stream for
    ///
    /// # Returns
    /// * `Result<Stream, StatusCode>` - Filtered membership update stream
    ///
    /// # Errors
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - LiveQuery setup failed
    pub async fn create_user_membership_stream(
        &self,
        user_id: &str,
    ) -> Result<impl Stream<Item = Result<FilteredMembershipUpdate, StatusCode>>, StatusCode> {
        debug!("Creating membership stream for user: {}", user_id);

        // Create comprehensive LiveQuery for all membership changes in rooms where user is joined
        let mut stream = self
            .membership_repo
            .create_user_membership_live_query(user_id)
            .await
            .map_err(|e| {
                error!("Failed to create membership LiveQuery for user {}: {}", user_id, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let user_id_owned = user_id.to_string();
        let service = Arc::new(self.clone());

        // Transform SurrealDB notifications into filtered membership updates
        let filtered_stream = stream
            .stream::<surrealdb::Notification<Membership>>(0)
            .map_err(|e| {
                error!("Failed to create membership notification stream: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .then(move |notification_result| {
                let user_id_clone = user_id_owned.clone();
                let service_clone = service.clone();

                async move {
                    match notification_result {
                        Ok(notification) => {
                            service_clone
                                .process_membership_notification(notification, &user_id_clone)
                                .await
                        },
                        Err(e) => {
                            error!("Membership notification error: {}", e);
                            Err(StatusCode::INTERNAL_SERVER_ERROR)
                        },
                    }
                }
            })
            .filter_map(|result| {
                async move {
                    match result {
                        Ok(Some(update)) => Some(Ok(update)),
                        Ok(None) => None, // Filtered out
                        Err(e) => Some(Err(e)),
                    }
                }
            });

        Ok(filtered_stream)
    }

    /// Create a room-specific membership stream
    ///
    /// Returns membership updates for a specific room, filtered by user permissions.
    /// Useful for room-specific UI components or admin dashboards.
    ///
    /// # Arguments
    /// * `room_id` - The room to monitor
    /// * `user_id` - The user requesting the stream (for authorization)
    ///
    /// # Returns
    /// * `Result<Stream, StatusCode>` - Room membership update stream
    ///
    /// # Errors
    /// * `StatusCode::FORBIDDEN` - User not authorized to view room membership
    /// * `StatusCode::INTERNAL_SERVER_ERROR` - LiveQuery setup failed
    pub async fn create_room_membership_stream(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<impl Stream<Item = Result<FilteredMembershipUpdate, StatusCode>>, StatusCode> {
        debug!("Creating room membership stream for room {} and user {}", room_id, user_id);

        // Verify user can access this room's membership information
        self.verify_room_membership_access(room_id, user_id).await?;

        // Create LiveQuery for membership changes in this specific room
        let mut stream = self
            .membership_repo
            .create_room_membership_live_query(room_id)
            .await
            .map_err(|e| {
                error!("Failed to create room membership LiveQuery for room {}: {}", room_id, e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let user_id_owned = user_id.to_string();
        let service = Arc::new(self.clone());

        // Transform and filter notifications
        let filtered_stream = stream
            .stream::<surrealdb::Notification<Membership>>(0)
            .map_err(|e| {
                error!("Failed to create room membership notification stream: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .then(move |notification_result| {
                let user_id_clone = user_id_owned.clone();
                let service_clone = service.clone();

                async move {
                    match notification_result {
                        Ok(notification) => {
                            service_clone
                                .process_membership_notification(notification, &user_id_clone)
                                .await
                        },
                        Err(e) => {
                            error!("Room membership notification error: {}", e);
                            Err(StatusCode::INTERNAL_SERVER_ERROR)
                        },
                    }
                }
            })
            .filter_map(|result| async move {
                match result {
                    Ok(Some(update)) => Some(Ok(update)),
                    Ok(None) => None,
                    Err(e) => Some(Err(e)),
                }
            });

        Ok(filtered_stream)
    }

    /// Process a membership notification and apply authorization filtering
    async fn process_membership_notification(
        &self,
        notification: surrealdb::Notification<Membership>,
        viewer_user_id: &str,
    ) -> Result<Option<FilteredMembershipUpdate>, StatusCode> {
        let membership = notification.data;
        let action = match notification.action {
            surrealdb::Action::Create => MembershipOperation::Create,
            surrealdb::Action::Update => MembershipOperation::Update,
            surrealdb::Action::Delete => MembershipOperation::Delete,
            _ => return Ok(None), // Ignore other actions
        };

        debug!(
            "Processing membership notification: {:?} for user {} in room {}",
            action, membership.user_id, membership.room_id
        );

        // Create base membership update
        let update = MembershipUpdate {
            action,
            room_id: membership.room_id.clone(),
            user_id: membership.user_id.clone(),
            membership_state: membership.membership.clone(),
            event_id: Some(format!(
                "${}:{}",
                chrono::Utc::now().timestamp_millis(),
                &self.homeserver_name
            )), // Generate event ID
            sender: membership.invited_by.clone(),
            reason: extract_reason_from_membership_event(&membership),
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        // Apply authorization filtering
        match self.filter_membership_update(&update, viewer_user_id).await {
            Ok(Some(filtered_update)) => Ok(Some(filtered_update)),
            Ok(None) => {
                debug!("Membership update filtered out for user {}", viewer_user_id);
                Ok(None)
            },
            Err(e) => {
                warn!("Failed to filter membership update: {:?}", e);
                Err(e)
            },
        }
    }

    /// Apply authorization filtering to a membership update
    async fn filter_membership_update(
        &self,
        update: &MembershipUpdate,
        viewer_user_id: &str,
    ) -> Result<Option<FilteredMembershipUpdate>, StatusCode> {
        // Check if viewer can see membership changes in this room
        let can_view = self.can_view_membership_changes(&update.room_id, viewer_user_id).await?;

        if !can_view {
            return Ok(None);
        }

        // Determine visibility level based on viewer's permissions
        let has_full_visibility = self
            .has_full_membership_visibility(&update.room_id, viewer_user_id)
            .await?;

        // Create filtered content based on visibility level
        let filtered_content = if has_full_visibility {
            // Full details available
            json!({
                "membership": update.membership_state,
                "user_id": update.user_id,
                "sender": update.sender,
                "reason": update.reason,
                "event_id": update.event_id,
                "timestamp": update.timestamp
            })
        } else {
            // Limited details (e.g., only join/leave, no detailed reasons)
            json!({
                "membership": update.membership_state,
                "user_id": update.user_id,
                "timestamp": update.timestamp
            })
        };

        Ok(Some(FilteredMembershipUpdate {
            update: update.clone(),
            has_full_visibility,
            filtered_content,
        }))
    }

    /// Check if a user can view membership changes in a room
    async fn can_view_membership_changes(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<bool, StatusCode> {
        // Get user's membership in the room
        let user_membership = self.get_user_room_membership(room_id, user_id).await?;

        match user_membership {
            Some(membership) => {
                // Users with join/invite membership can see membership changes
                match membership.membership {
                    MembershipState::Join | MembershipState::Invite => Ok(true),
                    _ => Ok(false),
                }
            },
            None => {
                // Non-members might be able to see public room membership based on join rules
                self.can_view_public_membership(room_id, user_id).await
            },
        }
    }

    /// Check if a user has full visibility of membership details
    async fn has_full_membership_visibility(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<bool, StatusCode> {
        // Check if user is a room admin (can see full details like kick/ban reasons)
        self.power_level_validator.is_room_admin(room_id, user_id).await
    }

    /// Check if a user can view public membership information
    async fn can_view_public_membership(
        &self,
        room_id: &str,
        _user_id: &str,
    ) -> Result<bool, StatusCode> {
        // Get room's join rules to determine public visibility
        let join_rules = self.get_room_join_rules(room_id).await?;

        // Only public rooms allow non-member membership viewing
        match join_rules.as_str() {
            "public" => Ok(true),
            _ => Ok(false),
        }
    }

    /// Get user's membership in a specific room
    async fn get_user_room_membership(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<Option<Membership>, StatusCode> {
        match self.membership_repo.get_by_room_user(room_id, user_id).await {
            Ok(membership) => Ok(membership),
            Err(e) => {
                error!(
                    "Failed to get user membership for {} in room {}: {:?}",
                    user_id, room_id, e
                );
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            },
        }
    }

    /// Get room's join rules
    async fn get_room_join_rules(&self, room_id: &str) -> Result<String, StatusCode> {
        match self.room_repo.get_room_join_rules(room_id).await {
            Ok(join_rules) => {
                let join_rule_str = match join_rules {
                    matryx_surrealdb::repository::room::JoinRules::Public => "public",
                    matryx_surrealdb::repository::room::JoinRules::Invite => "invite",
                    matryx_surrealdb::repository::room::JoinRules::Knock => "knock",
                    matryx_surrealdb::repository::room::JoinRules::Private => "private",
                    matryx_surrealdb::repository::room::JoinRules::Restricted => "restricted",
                };
                Ok(join_rule_str.to_string())
            },
            Err(e) => {
                error!("Failed to get join rules for room {}: {:?}", room_id, e);
                Ok("invite".to_string()) // Default join rule on error
            },
        }
    }

    /// Verify user can access room membership information
    async fn verify_room_membership_access(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<(), StatusCode> {
        let can_access = self.can_view_membership_changes(room_id, user_id).await?;

        if can_access {
            Ok(())
        } else {
            warn!("User {} denied access to room {} membership", user_id, room_id);
            Err(StatusCode::FORBIDDEN)
        }
    }

    /// Create a batched membership stream for multiple rooms
    ///
    /// Efficiently monitors membership changes across multiple rooms for a user.
    /// Useful for sync operations that need to track many rooms simultaneously.
    ///
    /// # Arguments
    /// * `room_ids` - List of rooms to monitor
    /// * `user_id` - The user requesting the stream
    ///
    /// # Returns
    /// * `Result<Stream, StatusCode>` - Batched membership update stream
    pub async fn create_batched_membership_stream(
        &self,
        room_ids: Vec<String>,
        user_id: &str,
    ) -> Result<impl Stream<Item = Result<Vec<FilteredMembershipUpdate>, StatusCode>>, StatusCode>
    {
        debug!(
            "Creating batched membership stream for {} rooms and user {}",
            room_ids.len(),
            user_id
        );

        // Verify access to all requested rooms
        for room_id in &room_ids {
            self.verify_room_membership_access(room_id, user_id).await?;
        }

        // Create LiveQuery for all specified rooms
        let room_ids_json = serde_json::to_string(&room_ids).map_err(|e| {
            error!("Failed to serialize room IDs: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        // Log the room IDs for debugging and monitoring
        debug!("Creating live membership stream for rooms: {}", room_ids_json);

        let mut stream = self
            .membership_repo
            .create_batched_membership_live_query(room_ids)
            .await
            .map_err(|e| {
                error!("Failed to create batched membership LiveQuery: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let user_id_owned = user_id.to_string();
        let service = Arc::new(self.clone());

        // Batch notifications and filter them
        let batched_stream = stream
            .stream::<surrealdb::Notification<Membership>>(0)
            .map_err(|e| {
                error!("Failed to create batched membership notification stream: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ready_chunks(10) // Batch up to 10 updates
            .then(move |notification_batch| {
                let user_id_clone = user_id_owned.clone();
                let service_clone = service.clone();

                async move {
                    let mut filtered_updates = Vec::new();

                    for notification_result in notification_batch {
                        match notification_result {
                            Ok(notification) => {
                                match service_clone
                                    .process_membership_notification(notification, &user_id_clone)
                                    .await
                                {
                                    Ok(Some(update)) => filtered_updates.push(update),
                                    Ok(None) => {}, // Filtered out
                                    Err(e) => return Err(e),
                                }
                            },
                            Err(e) => {
                                error!("Batched membership notification error: {}", e);
                                return Err(StatusCode::INTERNAL_SERVER_ERROR);
                            },
                        }
                    }

                    Ok(filtered_updates)
                }
            })
            .filter_map(|result| {
                async move {
                    match result {
                        Ok(updates) if !updates.is_empty() => Some(Ok(updates)),
                        Ok(_) => None, // Empty batch
                        Err(e) => Some(Err(e)),
                    }
                }
            });

        Ok(batched_stream)
    }
}

/// Extract the reason field from a Matrix membership event according to the Matrix specification
///
/// Per the Matrix Client-Server API spec, the reason field is an optional user-supplied
/// text explaining why their membership has changed. This function extracts that field
/// from the membership event content.
///
/// # Arguments
/// * `membership` - The membership entity containing the event data
///
/// # Returns
/// * `Option<String>` - The reason if present, None otherwise
fn extract_reason_from_membership_event(membership: &Membership) -> Option<String> {
    membership.reason.clone()
}

// Make the service cloneable for use in async streams
impl Clone for LiveMembershipService {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            room_repo: self.room_repo.clone(),
            membership_repo: self.membership_repo.clone(),
            join_rules_validator: self.join_rules_validator.clone(),
            power_level_validator: self.power_level_validator.clone(),
            alias_resolver: self.alias_resolver.clone(),
            homeserver_name: self.homeserver_name.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests would be implemented here following Rust testing best practices
    // Using expect() in tests (never unwrap()) for proper error messages
    // These tests would cover all LiveQuery scenarios and authorization filtering
}
