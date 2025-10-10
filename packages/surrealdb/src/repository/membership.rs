use crate::repository::error::RepositoryError;
use crate::repository::state_resolution::StateResolver;
use crate::repository::{EventRepository, RoomRepository};
use futures_util::{Stream, StreamExt};
use matryx_entity::types::{Event, Membership, MembershipState};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use surrealdb::{Surreal, engine::any::Any};
use tracing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipContext {
    pub user_id: String,
    pub room_id: String,
    pub membership: MembershipState,
    pub sender: String,
    pub reason: Option<String>,
    pub invited_by: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub is_direct: Option<bool>,
    pub third_party_invite: Option<serde_json::Value>,
    pub join_authorised_via_users_server: Option<String>,
    pub event_id: String,
    pub origin_server_ts: i64,
    pub auth_events: Vec<String>,
    pub prev_events: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationMember {
    pub user_id: String,
    pub membership: MembershipState,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub reason: Option<String>,
    pub invited_by: Option<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone)]
pub struct MembershipRepository {
    db: Surreal<Any>,
}

impl MembershipRepository {
    pub fn new(db: Surreal<Any>) -> Self {
        Self { db }
    }

    pub async fn create(&self, membership: &Membership) -> Result<Membership, RepositoryError> {
        let membership_clone = membership.clone();
        let id = format!("{}:{}", membership.room_id, membership.user_id);
        let created: Option<Membership> =
            self.db.create(("room_membership", id)).content(membership_clone).await?;

        created.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to create membership"))
        })
    }

    pub async fn get_by_room_user(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<Option<Membership>, RepositoryError> {
        let id = format!("{}:{}", room_id, user_id);
        let membership: Option<Membership> = self.db.select(("membership", id)).await?;
        Ok(membership)
    }

    pub async fn get_room_members(
        &self,
        room_id: &str,
    ) -> Result<Vec<Membership>, RepositoryError> {
        let room_id_owned = room_id.to_string();
        let memberships: Vec<Membership> = self
            .db
            .query("SELECT * FROM room_membership WHERE room_id = $room_id AND membership = 'join'")
            .bind(("room_id", room_id_owned))
            .await?
            .take(0)?;
        Ok(memberships)
    }

    pub async fn get_user_rooms(&self, user_id: &str) -> Result<Vec<Membership>, RepositoryError> {
        let user_id_owned = user_id.to_string();
        let memberships: Vec<Membership> = self
            .db
            .query("SELECT * FROM room_membership WHERE user_id = $user_id AND membership = 'join'")
            .bind(("user_id", user_id_owned))
            .await?
            .take(0)?;
        Ok(memberships)
    }

    pub async fn get_user_rooms_by_state(
        &self,
        user_id: &str,
        membership_state: &str,
    ) -> Result<Vec<Membership>, RepositoryError> {
        let user_id_owned = user_id.to_string();
        let state_owned = membership_state.to_string();
        let memberships: Vec<Membership> = self
            .db
            .query("SELECT * FROM room_membership WHERE user_id = $user_id AND membership = $state")
            .bind(("user_id", user_id_owned))
            .bind(("state", state_owned))
            .await?
            .take(0)?;
        Ok(memberships)
    }

    pub async fn update(&self, membership: &Membership) -> Result<Membership, RepositoryError> {
        let membership_clone = membership.clone();
        let id = format!("{}:{}", membership.room_id, membership.user_id);
        let updated: Option<Membership> =
            self.db.update(("room_membership", id)).content(membership_clone).await?;

        updated.ok_or_else(|| {
            RepositoryError::Database(surrealdb::Error::msg("Failed to update membership"))
        })
    }

    /// Subscribe to real-time room membership changes using SurrealDB LiveQuery
    /// Returns a stream of notifications for membership changes in the specified room
    /// Note: Filtering must be done client-side as SurrealDB LiveQuery doesn't support WHERE clauses
    pub async fn subscribe_room_membership(
        &self,
        _room_id: &str,
    ) -> Result<impl Stream<Item = Result<Membership, RepositoryError>> + use<>, RepositoryError>
    {
        // Create SurrealDB LiveQuery for memberships in the table
        let mut stream = self
            .db
            .query("LIVE SELECT * FROM membership")
            .await
            .map_err(RepositoryError::Database)?;

        // Transform SurrealDB notification stream to membership stream
        let membership_stream = stream
            .stream::<surrealdb::Notification<Membership>>(0)
            .map_err(RepositoryError::Database)?
            .map(|notification_result| -> Result<Membership, RepositoryError> {
                let notification = notification_result.map_err(RepositoryError::Database)?;

                match notification.action {
                    surrealdb::Action::Create | surrealdb::Action::Update => Ok(notification.data),
                    surrealdb::Action::Delete => {
                        // For deleted memberships, return the data for proper handling
                        Ok(notification.data)
                    },
                    _ => {
                        Err(RepositoryError::Database(surrealdb::Error::msg(format!(
                            "Unexpected action in membership notification: {:?}",
                            notification.action
                        ))))
                    },
                }
            });

        Ok(membership_stream)
    }

    /// Subscribe to real-time user membership changes using SurrealDB LiveQuery
    /// Returns a stream of notifications for membership changes for the specified user
    /// Note: Filtering must be done client-side as SurrealDB LiveQuery doesn't support WHERE clauses
    pub async fn subscribe_user_membership(
        &self,
        _user_id: &str,
    ) -> Result<impl Stream<Item = Result<Membership, RepositoryError>> + use<>, RepositoryError>
    {
        // Create SurrealDB LiveQuery for memberships in the table
        let mut stream = self
            .db
            .query("LIVE SELECT * FROM membership")
            .await
            .map_err(RepositoryError::Database)?;

        // Transform SurrealDB notification stream to membership stream
        let membership_stream = stream
            .stream::<surrealdb::Notification<Membership>>(0)
            .map_err(RepositoryError::Database)?
            .map(|notification_result| -> Result<Membership, RepositoryError> {
                let notification = notification_result.map_err(RepositoryError::Database)?;

                match notification.action {
                    surrealdb::Action::Create | surrealdb::Action::Update => Ok(notification.data),
                    surrealdb::Action::Delete => {
                        // For deleted memberships, return the data for proper handling
                        Ok(notification.data)
                    },
                    _ => {
                        Err(RepositoryError::Database(surrealdb::Error::msg(format!(
                            "Unexpected action in membership notification: {:?}",
                            notification.action
                        ))))
                    },
                }
            });

        Ok(membership_stream)
    }

    /// Get essential members for lazy loading with database-level optimization
    pub async fn get_essential_members_optimized(
        &self,
        room_id: &str,
        user_id: &str,
        timeline_senders: &[String],
    ) -> Result<Vec<Membership>, RepositoryError> {
        // Single optimized query instead of post-filtering
        let query = r#"
            SELECT * FROM room_membership
            WHERE room_id = $room_id
            AND membership = 'join'
            AND (
                user_id = $user_id                    -- Always include requesting user
                OR user_id IN $timeline_senders       -- Include timeline event senders
                OR user_id IN (                       -- Include power users (admins/moderators)
                    SELECT user_id FROM power_levels
                    WHERE room_id = $room_id AND power_level >= 50
                )
                OR user_id = (                        -- Include room creator
                    SELECT creator FROM rooms WHERE room_id = $room_id
                )
            )
            ORDER BY power_level DESC, join_time ASC
        "#;

        let mut response = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("timeline_senders", timeline_senders.to_vec()))
            .await?;

        let memberships: Vec<Membership> = response.take(0)?;
        Ok(memberships)
    }

    /// Get room power level hierarchy for lazy loading optimization
    pub async fn get_room_power_hierarchy(
        &self,
        room_id: &str,
    ) -> Result<Vec<(String, i64)>, RepositoryError> {
        let query = r#"
            SELECT user_id, power_level FROM power_levels
            WHERE room_id = $room_id AND power_level >= 50
            ORDER BY power_level DESC
        "#;

        let mut response = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let hierarchy: Vec<(String, i64)> = response.take(0)?;
        Ok(hierarchy)
    }

    /// Get room creator with caching
    pub async fn get_room_creator_cached(
        &self,
        room_id: &str,
        cache: &Cache<String, Option<String>>,
    ) -> Result<Option<String>, RepositoryError> {
        if let Some(cached_creator) = cache.get(room_id).await {
            return Ok(cached_creator);
        }

        let query = "SELECT creator FROM rooms WHERE room_id = $room_id";
        let mut response = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let creator: Option<String> = response.take(0)?;
        cache.insert(room_id.to_string(), creator.clone()).await;

        Ok(creator)
    }

    /// Enhanced LiveQuery stream for real-time membership monitoring with cache invalidation
    pub async fn subscribe_room_membership_enhanced(
        &self,
        room_id: &str,
        cache_invalidation_tx: tokio::sync::broadcast::Sender<String>,
    ) -> Result<impl futures_util::Stream<Item = Vec<Membership>> + Send + 'static, RepositoryError>
    {
        // Use the same API pattern as the existing methods
        let stream = self.db.select("room_membership").live().await?;

        // Convert to owned to avoid lifetime issues
        let room_id_owned = room_id.to_string();

        // Return a stream that filters for the room and triggers cache invalidation
        let invalidating_stream = stream.map(move |notification_result| {
            // Extract the actual data from the notification result
            let memberships: Vec<Membership> = match notification_result {
                Ok(notification) => notification.data,
                Err(_) => Vec::new(),
            };

            // Filter memberships for the specific room (client-side filtering as noted in existing methods)
            let room_memberships: Vec<Membership> = memberships
                .into_iter()
                .filter(|membership| membership.room_id == room_id_owned)
                .collect();

            // Trigger cache invalidation for this room if there are relevant changes
            if !room_memberships.is_empty() {
                if let Err(e) = cache_invalidation_tx.send(room_id_owned.clone()) {
                    tracing::warn!(room_id = %room_id_owned, error = %e, "Failed to send cache invalidation signal");
                } else {
                    tracing::debug!(room_id = %room_id_owned, changes = room_memberships.len(), "Cache invalidation triggered by membership changes");
                }
            }

            room_memberships
        });

        Ok(invalidating_stream)
    }

    /// Enhanced LiveQuery stream for power level changes with cache invalidation
    pub async fn subscribe_power_levels_enhanced(
        &self,
        room_id: &str,
        cache_invalidation_tx: tokio::sync::broadcast::Sender<String>,
    ) -> Result<
        impl futures_util::Stream<Item = Vec<serde_json::Value>> + Send + 'static,
        RepositoryError,
    > {
        // Use SurrealDB's live select API - we'll need to select from power_levels table
        let stream = self.db.select("power_levels").live().await?;

        let room_id_owned = room_id.to_string();

        // Return a stream that filters for the room and triggers cache invalidation
        let invalidating_stream = stream.map(move |notification_result| {
            // Extract the actual data from the notification result
            let power_levels: Vec<serde_json::Value> = match notification_result {
                Ok(notification) => notification.data,
                Err(_) => Vec::new(),
            };

            // Filter power levels for the specific room (client-side filtering)
            let room_power_levels: Vec<serde_json::Value> = power_levels
                .into_iter()
                .filter_map(|power_level| {
                    // Convert to serde_json::Value for filtering
                    let value = serde_json::to_value(power_level).ok()?;
                    // Extract room_id from the power level entry for filtering
                    if value.get("room_id")
                        .and_then(|v| v.as_str())
                        .map(|rid| rid == room_id_owned)
                        .unwrap_or(false)
                    {
                        Some(value)
                    } else {
                        None
                    }
                })
                .collect();

            // Trigger cache invalidation for this room if there are relevant changes
            if !room_power_levels.is_empty() {
                if let Err(e) = cache_invalidation_tx.send(room_id_owned.clone()) {
                    tracing::warn!(room_id = %room_id_owned, error = %e, "Failed to send power level cache invalidation signal");
                } else {
                    tracing::debug!(room_id = %room_id_owned, changes = room_power_levels.len(), "Power level cache invalidation triggered");
                }
            }

            room_power_levels
        });

        Ok(invalidating_stream)
    }

    /// Apply database indexes for lazy loading optimization
    pub async fn apply_lazy_loading_indexes(&self) -> Result<(), RepositoryError> {
        // Apply indexes for room_membership table
        self.db.query("DEFINE INDEX idx_room_membership_room_id_membership ON TABLE room_membership COLUMNS room_id, membership").await?;
        self.db
            .query(
                "DEFINE INDEX idx_room_membership_user_id ON TABLE room_membership COLUMNS user_id",
            )
            .await?;
        self.db.query("DEFINE INDEX idx_room_membership_composite ON TABLE room_membership COLUMNS room_id, membership, user_id").await?;

        // Apply indexes for power_levels table
        self.db.query("DEFINE INDEX idx_power_levels_room_id_level ON TABLE power_levels COLUMNS room_id, power_level").await?;

        // Apply indexes for rooms table
        self.db
            .query("DEFINE INDEX idx_rooms_room_id ON TABLE rooms COLUMNS room_id")
            .await?;

        // Apply indexes for ordering
        self.db.query("DEFINE INDEX idx_room_membership_join_time ON TABLE room_membership COLUMNS join_time").await?;
        self.db.query("DEFINE INDEX idx_room_membership_ordering ON TABLE room_membership COLUMNS power_level, join_time").await?;

        Ok(())
    }

    /// Performance benchmark for database queries - ensure sub-50ms response for 10k+ member rooms
    pub async fn benchmark_essential_members_query(
        &self,
        room_id: &str,
        user_id: &str,
        timeline_senders: &[String],
    ) -> Result<(Vec<Membership>, Duration), RepositoryError> {
        let start_time = std::time::Instant::now();

        let members = self
            .get_essential_members_optimized(room_id, user_id, timeline_senders)
            .await?;

        let elapsed = start_time.elapsed();
        Ok((members, elapsed))
    }

    /// Get membership for a specific user in a room
    pub async fn get_membership(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<Option<Membership>, RepositoryError> {
        self.get_by_room_user(room_id, user_id).await
    }

    /// Create membership (alias for create for consistency)
    pub async fn create_membership(
        &self,
        membership: &Membership,
    ) -> Result<Membership, RepositoryError> {
        self.create(membership).await
    }

    /// Update membership (alias for update for consistency)
    pub async fn update_membership(
        &self,
        membership: &Membership,
    ) -> Result<Membership, RepositoryError> {
        self.update(membership).await
    }

    /// Get user rooms as room IDs only
    pub async fn get_user_room_ids(&self, user_id: &str) -> Result<Vec<String>, RepositoryError> {
        let memberships = self.get_user_rooms(user_id).await?;
        Ok(memberships.into_iter().map(|m| m.room_id).collect())
    }

    /// Check if user is in room
    pub async fn is_user_in_room(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<bool, RepositoryError> {
        let membership = self.get_membership(room_id, user_id).await?;
        match membership {
            Some(m) => Ok(m.membership.to_string() == "join"),
            None => Ok(false),
        }
    }

    /// Kick a user from a room
    pub async fn kick_user(
        &self,
        room_id: &str,
        user_id: &str,
        _kicker: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Leave,
            reason,
            invited_by: None,
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        self.update_membership(&membership).await?;
        Ok(())
    }

    /// Ban a user from a room
    pub async fn ban_user(
        &self,
        room_id: &str,
        user_id: &str,
        _banner: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Ban,
            reason,
            invited_by: None,
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        self.update_membership(&membership).await?;
        Ok(())
    }

    /// Unban a user from a room
    pub async fn unban_user(
        &self,
        room_id: &str,
        user_id: &str,
        _unbanner: &str,
    ) -> Result<(), RepositoryError> {
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Leave,
            reason: Some("Unbanned".to_string()),
            invited_by: None,
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        self.update_membership(&membership).await?;
        Ok(())
    }

    /// Invite a user to a room
    pub async fn invite_user(
        &self,
        room_id: &str,
        user_id: &str,
        inviter: &str,
    ) -> Result<(), RepositoryError> {
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Invite,
            reason: None,
            invited_by: Some(inviter.to_string()),
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        self.create_membership(&membership).await?;
        Ok(())
    }

    /// Knock on a room (request to join)
    pub async fn knock_on_room(
        &self,
        room_id: &str,
        user_id: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Knock,
            reason,
            invited_by: None,
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        self.create_membership(&membership).await?;
        Ok(())
    }

    /// Validate a membership change
    pub async fn validate_membership_change(
        &self,
        room_id: &str,
        user_id: &str,
        target_user: &str,
        new_membership: MembershipState,
    ) -> Result<bool, RepositoryError> {
        // Check if the user making the change is a member of the room
        if !self.is_user_in_room(room_id, user_id).await? {
            return Ok(false);
        }

        // Get current membership of target user
        let current_membership = self.get_membership(room_id, target_user).await?;

        // Basic validation rules (simplified)
        match new_membership {
            MembershipState::Ban => {
                // Can't ban someone who is already banned
                if let Some(current) = current_membership
                    && current.membership == MembershipState::Ban {
                    return Ok(false);
                }
                Ok(true)
            },
            MembershipState::Leave => {
                // Can kick someone who is joined or invited
                if let Some(current) = current_membership {
                    Ok(current.membership == MembershipState::Join ||
                        current.membership == MembershipState::Invite)
                } else {
                    Ok(false)
                }
            },
            MembershipState::Invite => {
                // Can invite someone who is not already a member
                if let Some(current) = current_membership {
                    Ok(current.membership != MembershipState::Join &&
                        current.membership != MembershipState::Invite)
                } else {
                    Ok(true)
                }
            },
            MembershipState::Join => {
                // Can join if invited or if room allows
                Ok(true) // Simplified - would check room join rules
            },
            MembershipState::Knock => {
                // Can knock if not already a member
                if let Some(current) = current_membership {
                    Ok(current.membership != MembershipState::Join)
                } else {
                    Ok(true)
                }
            },
        }
    }

    // Federation membership methods

    /// Process federation join event
    pub async fn process_federation_join(
        &self,
        room_id: &str,
        user_id: &str,
        _event: &Event,
        origin: &str,
    ) -> Result<(), RepositoryError> {
        // Validate that the event is from the correct origin server
        let user_server = if let Some(colon_pos) = user_id.rfind(':') {
            &user_id[colon_pos + 1..]
        } else {
            return Err(RepositoryError::Validation {
                field: "user_id".to_string(),
                message: "Invalid user ID format".to_string(),
            });
        };

        if user_server != origin {
            return Err(RepositoryError::Validation {
                field: "origin".to_string(),
                message: "User server does not match origin".to_string(),
            });
        }

        // Create or update membership
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Join,
            reason: None,
            invited_by: None,
            updated_at: Some(chrono::Utc::now()),
            display_name: None, // Would extract from event content
            avatar_url: None,   // Would extract from event content
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: Some(origin.to_string()),
        };

        self.update_membership(&membership).await?;
        Ok(())
    }

    /// Process federation leave event
    pub async fn process_federation_leave(
        &self,
        room_id: &str,
        user_id: &str,
        _event: &Event,
        origin: &str,
    ) -> Result<(), RepositoryError> {
        // Validate origin
        let user_server = if let Some(colon_pos) = user_id.rfind(':') {
            &user_id[colon_pos + 1..]
        } else {
            return Err(RepositoryError::Validation {
                field: "user_id".to_string(),
                message: "Invalid user ID format".to_string(),
            });
        };

        if user_server != origin {
            return Err(RepositoryError::Validation {
                field: "origin".to_string(),
                message: "User server does not match origin".to_string(),
            });
        }

        // Update membership to leave
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Leave,
            reason: None, // Would extract from event content
            invited_by: None,
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        self.update_membership(&membership).await?;
        Ok(())
    }

    /// Process federation invite event
    pub async fn process_federation_invite(
        &self,
        room_id: &str,
        user_id: &str,
        event: &Event,
        origin: &str,
    ) -> Result<(), RepositoryError> {
        // For invites, the origin should be the inviter's server, not necessarily the invitee's server
        let inviter = &event.sender;
        let inviter_server = if let Some(colon_pos) = inviter.rfind(':') {
            &inviter[colon_pos + 1..]
        } else {
            return Err(RepositoryError::Validation {
                field: "sender".to_string(),
                message: "Invalid sender format".to_string(),
            });
        };

        if inviter_server != origin {
            return Err(RepositoryError::Validation {
                field: "origin".to_string(),
                message: "Inviter server does not match origin".to_string(),
            });
        }

        // Create invite membership
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Invite,
            reason: None,
            invited_by: Some(inviter.clone()),
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        self.create_membership(&membership).await?;
        Ok(())
    }

    /// Process federation knock event
    pub async fn process_federation_knock(
        &self,
        room_id: &str,
        user_id: &str,
        _event: &Event,
        origin: &str,
    ) -> Result<(), RepositoryError> {
        // Validate origin
        let user_server = if let Some(colon_pos) = user_id.rfind(':') {
            &user_id[colon_pos + 1..]
        } else {
            return Err(RepositoryError::Validation {
                field: "user_id".to_string(),
                message: "Invalid user ID format".to_string(),
            });
        };

        if user_server != origin {
            return Err(RepositoryError::Validation {
                field: "origin".to_string(),
                message: "User server does not match origin".to_string(),
            });
        }

        // Create knock membership
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Knock,
            reason: None, // Would extract from event content
            invited_by: None,
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        self.create_membership(&membership).await?;
        Ok(())
    }

    /// Validate federation membership change
    pub async fn validate_federation_membership_change(
        &self,
        _room_id: &str,
        event: &Event,
        _origin: &str,
    ) -> Result<bool, RepositoryError> {
        // Basic validation
        if event.event_type != "m.room.member" {
            return Ok(false);
        }

        let target_user = event.state_key.as_ref().ok_or_else(|| {
            RepositoryError::Validation {
                field: "state_key".to_string(),
                message: "Missing state key for membership event".to_string(),
            }
        })?;

        // Validate that the sender can perform this action
        let sender = &event.sender;

        // For self-membership changes (join, leave, knock), sender must be the target
        if let Ok(content) =
            serde_json::from_value::<serde_json::Value>(serde_json::to_value(&event.content)?)
            && let Some(membership) = content.get("membership").and_then(|v| v.as_str()) {
            match membership {
                "join" | "leave" | "knock" => {
                    if sender != target_user {
                        return Ok(false);
                    }
                },
                "invite" | "ban" => {
                    // For invites and bans, sender must have appropriate power level
                    // This would be validated by the calling code
                },
                _ => return Ok(false),
            }
        }

        Ok(true)
    }

    /// Get room members for federation
    pub async fn get_room_members_for_federation(
        &self,
        room_id: &str,
    ) -> Result<Vec<FederationMember>, RepositoryError> {
        let query = "
            SELECT
                user_id,
                membership,
                display_name,
                avatar_url,
                reason,
                invited_by,
                updated_at
            FROM membership
            WHERE room_id = $room_id
            ORDER BY updated_at DESC
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let members_data: Vec<serde_json::Value> = result.take(0)?;

        let mut members = Vec::new();
        for member_data in members_data {
            if let Some(user_id) = member_data.get("user_id").and_then(|v| v.as_str()) {
                let membership_str =
                    member_data.get("membership").and_then(|v| v.as_str()).unwrap_or("leave");
                let membership = match membership_str {
                    "join" => MembershipState::Join,
                    "leave" => MembershipState::Leave,
                    "invite" => MembershipState::Invite,
                    "ban" => MembershipState::Ban,
                    "knock" => MembershipState::Knock,
                    _ => MembershipState::Leave,
                };

                let updated_at = member_data
                    .get("updated_at")
                    .and_then(|v| v.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(chrono::Utc::now);

                let member = FederationMember {
                    user_id: user_id.to_string(),
                    membership,
                    display_name: member_data
                        .get("display_name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    avatar_url: member_data
                        .get("avatar_url")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    reason: member_data
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    invited_by: member_data
                        .get("invited_by")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    updated_at,
                };
                members.push(member);
            }
        }

        Ok(members)
    }

    /// Get federation member count by server
    pub async fn get_federation_member_count_by_server(
        &self,
        room_id: &str,
    ) -> Result<std::collections::HashMap<String, u64>, RepositoryError> {
        let members = self.get_room_members_for_federation(room_id).await?;
        let mut server_counts = std::collections::HashMap::new();

        for member in members {
            if member.membership == MembershipState::Join
                && let Some(colon_pos) = member.user_id.rfind(':') {
                let server = &member.user_id[colon_pos + 1..];
                *server_counts.entry(server.to_string()).or_insert(0) += 1;
            }
        }

        Ok(server_counts)
    }

    /// Validate federation membership event content
    pub async fn validate_federation_membership_content(
        &self,
        event: &Event,
    ) -> Result<bool, RepositoryError> {
        // Validate that the event content is valid for a membership event
        if let Ok(content) =
            serde_json::from_value::<serde_json::Value>(serde_json::to_value(&event.content)?)
            && let Some(membership) = content.get("membership").and_then(|v| v.as_str()) {
            match membership {
                "join" | "leave" | "invite" | "ban" | "knock" => return Ok(true),
                _ => return Ok(false),
            }
        }

        Ok(false)
    }

    // ADVANCED MEMBERSHIP OPERATIONS - SUBTASK 6 EXTENSIONS

    /// Invite a user to a room with reason support
    pub async fn invite_user_to_room(
        &self,
        room_id: &str,
        user_id: &str,
        inviter_id: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Check if user is already a member
        let existing_membership = self.get_membership(room_id, user_id).await?;
        let has_existing_membership = existing_membership.is_some();
        if let Some(membership) = existing_membership {
            match membership.membership {
                MembershipState::Join => {
                    return Err(RepositoryError::Conflict {
                        message: "User is already joined to the room".to_string(),
                    });
                },
                MembershipState::Invite => {
                    return Err(RepositoryError::Conflict {
                        message: "User is already invited to the room".to_string(),
                    });
                },
                MembershipState::Ban => {
                    return Err(RepositoryError::Validation {
                        field: "membership".to_string(),
                        message: "Cannot invite banned user".to_string(),
                    });
                },
                _ => {}, // Can invite if left or knocked
            }
        }

        // Validate inviter has permission to invite
        if !self.is_user_in_room(room_id, inviter_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!(
                    "User {} not authorized to invite users to room {}",
                    inviter_id, room_id
                ),
            });
        }

        // Create invite membership
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Invite,
            reason,
            invited_by: Some(inviter_id.to_string()),
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        // Use create if no existing membership, update if exists
        if has_existing_membership {
            self.update_membership(&membership).await?;
        } else {
            self.create_membership(&membership).await?;
        }

        Ok(())
    }

    /// Ban a user from a room with reason support
    pub async fn ban_user_from_room(
        &self,
        room_id: &str,
        user_id: &str,
        banner_id: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Validate banner has permission to ban
        if !self.is_user_in_room(room_id, banner_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!(
                    "User {} not authorized to ban users from room {}",
                    banner_id, room_id
                ),
            });
        }

        // Check if user is already banned
        let existing_membership = self.get_membership(room_id, user_id).await?;
        if let Some(membership) = &existing_membership
            && membership.membership == MembershipState::Ban {
            return Err(RepositoryError::Conflict {
                message: "User is already banned from the room".to_string(),
            });
        }

        // Create ban membership
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Ban,
            reason,
            invited_by: None,
            updated_at: Some(chrono::Utc::now()),
            display_name: existing_membership.as_ref().and_then(|m| m.display_name.clone()),
            avatar_url: existing_membership.as_ref().and_then(|m| m.avatar_url.clone()),
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        // Use create if no existing membership, update if exists
        if existing_membership.is_some() {
            self.update_membership(&membership).await?;
        } else {
            self.create_membership(&membership).await?;
        }

        Ok(())
    }

    /// Leave a room with reason support
    pub async fn leave_room(
        &self,
        room_id: &str,
        user_id: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Check if user is currently in the room
        let existing_membership =
            self.get_membership(room_id, user_id).await?.ok_or_else(|| {
                RepositoryError::NotFound {
                    entity_type: "Membership".to_string(),
                    id: format!("{}:{}", room_id, user_id),
                }
            })?;

        match existing_membership.membership {
            MembershipState::Join | MembershipState::Invite | MembershipState::Knock => {
                // Can leave from these states
            },
            MembershipState::Leave => {
                return Err(RepositoryError::Conflict {
                    message: "User has already left the room".to_string(),
                });
            },
            MembershipState::Ban => {
                return Err(RepositoryError::Validation {
                    field: "membership".to_string(),
                    message: "Cannot leave room while banned".to_string(),
                });
            },
        }

        // Update membership to leave
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Leave,
            reason,
            invited_by: existing_membership.invited_by,
            updated_at: Some(chrono::Utc::now()),
            display_name: existing_membership.display_name,
            avatar_url: existing_membership.avatar_url,
            is_direct: existing_membership.is_direct,
            third_party_invite: existing_membership.third_party_invite,
            join_authorised_via_users_server: existing_membership.join_authorised_via_users_server,
        };

        self.update_membership(&membership).await?;
        Ok(())
    }

    /// Forget room membership (remove from user's room list)
    pub async fn forget_room_membership(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<(), RepositoryError> {
        // Check if user has left or been banned from the room
        let existing_membership =
            self.get_membership(room_id, user_id).await?.ok_or_else(|| {
                RepositoryError::NotFound {
                    entity_type: "Membership".to_string(),
                    id: format!("{}:{}", room_id, user_id),
                }
            })?;

        match existing_membership.membership {
            MembershipState::Leave | MembershipState::Ban => {
                // Can forget from these states
            },
            MembershipState::Join | MembershipState::Invite | MembershipState::Knock => {
                return Err(RepositoryError::Validation {
                    field: "membership".to_string(),
                    message: "Cannot forget room while still a member. Leave the room first."
                        .to_string(),
                });
            },
        }

        // Mark the membership as forgotten by deleting the record
        let id = format!("{}:{}", room_id, user_id);
        let _: Option<Membership> = self.db.delete(("membership", id)).await?;

        Ok(())
    }

    /// Join a room (accept invitation or direct join)
    pub async fn join_room(
        &self,
        room_id: &str,
        user_id: &str,
        display_name: Option<String>,
        avatar_url: Option<String>,
        auth_service: &crate::repository::room_authorization::RoomAuthorizationService,
    ) -> Result<(), RepositoryError> {
        let existing_membership = self.get_membership(room_id, user_id).await?;

        // Validate the user can join
        if let Some(membership) = &existing_membership {
            match membership.membership {
                MembershipState::Join => {
                    return Err(RepositoryError::Conflict {
                        message: "User is already joined to the room".to_string(),
                    });
                },
                MembershipState::Ban => {
                    return Err(RepositoryError::Validation {
                        field: "membership".to_string(),
                        message: "Cannot join room while banned".to_string(),
                    });
                },
                MembershipState::Invite => {
                    // Can join from invite
                },
                MembershipState::Leave | MembershipState::Knock => {
                    // Validate join rules before allowing join
                    let auth_result = auth_service
                        .validate_join_request(room_id, user_id, None)
                        .await?;

                    if !auth_result.authorized {
                        return Err(RepositoryError::Forbidden {
                            reason: auth_result.reason.unwrap_or_else(|| "Cannot join room".to_string()),
                        });
                    }
                },
            }
        }

        // Create join membership
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Join,
            reason: None,
            invited_by: existing_membership.as_ref().and_then(|m| m.invited_by.clone()),
            updated_at: Some(chrono::Utc::now()),
            display_name,
            avatar_url,
            is_direct: existing_membership.as_ref().and_then(|m| m.is_direct),
            third_party_invite: existing_membership
                .as_ref()
                .and_then(|m| m.third_party_invite.clone()),
            join_authorised_via_users_server: None,
        };

        // Use create if no existing membership, update if exists
        if existing_membership.is_some() {
            self.update_membership(&membership).await?;
        } else {
            self.create_membership(&membership).await?;
        }

        Ok(())
    }

    /// Get users with specific membership state
    pub async fn get_users_by_membership_state(
        &self,
        room_id: &str,
        membership_state: MembershipState,
    ) -> Result<Vec<String>, RepositoryError> {
        let query = "SELECT user_id FROM membership WHERE room_id = $room_id AND membership = $membership_state";
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("membership_state", membership_state.to_string()))
            .await?;
        let user_data: Vec<serde_json::Value> = result.take(0)?;

        let user_ids: Vec<String> = user_data
            .into_iter()
            .filter_map(|v| v.get("user_id").and_then(|id| id.as_str()).map(|s| s.to_string()))
            .collect();

        Ok(user_ids)
    }

    /// Get users with specific membership state at a specific event
    pub async fn get_users_by_membership_state_at_event(
        &self,
        room_id: &str,
        membership_state: MembershipState,
        at_event_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        // First get the timestamp of the target event
        let event_query = "SELECT created_at FROM event WHERE event_id = $event_id LIMIT 1";
        let mut event_result = self
            .db
            .query(event_query)
            .bind(("event_id", at_event_id.to_string()))
            .await?;
        let event_data: Vec<serde_json::Value> = event_result.take(0)?;
        
        if event_data.is_empty() {
            return Ok(Vec::new());
        }
        
        let target_timestamp = event_data[0]["created_at"].as_str()
            .ok_or_else(|| RepositoryError::Validation { 
                field: "created_at".to_string(),
                message: "Event timestamp not found".to_string() 
            })?;

        // Get users who had the specified membership state at or before that timestamp
        // For each user, get their most recent membership state at that time
        let query = r#"
            SELECT user_id FROM membership 
            WHERE room_id = $room_id 
            AND membership = $membership_state 
            AND created_at <= $target_timestamp
            AND user_id IN (
                SELECT user_id FROM membership AS m2 
                WHERE m2.room_id = $room_id 
                AND m2.user_id = membership.user_id 
                AND m2.created_at <= $target_timestamp
                ORDER BY m2.created_at DESC 
                LIMIT 1
            )
        "#;
        
        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("membership_state", membership_state.to_string()))
            .bind(("target_timestamp", target_timestamp.to_string()))
            .await?;
        let user_data: Vec<serde_json::Value> = result.take(0)?;

        let user_ids: Vec<String> = user_data
            .into_iter()
            .filter_map(|v| v.get("user_id").and_then(|id| id.as_str()).map(|s| s.to_string()))
            .collect();

        Ok(user_ids)
    }

    /// Get room members at a specific event
    pub async fn get_room_members_at_event(
        &self,
        room_id: &str,
        at_event_id: &str,
    ) -> Result<Vec<Membership>, RepositoryError> {
        // First get the timestamp of the target event
        let event_query = "SELECT created_at FROM event WHERE event_id = $event_id LIMIT 1";
        let mut event_result = self
            .db
            .query(event_query)
            .bind(("event_id", at_event_id.to_string()))
            .await?;
        let event_data: Vec<serde_json::Value> = event_result.take(0)?;
        
        if event_data.is_empty() {
            return Ok(Vec::new());
        }
        
        let target_timestamp = event_data[0]["created_at"].as_str()
            .ok_or_else(|| RepositoryError::Validation { 
                field: "created_at".to_string(),
                message: "Event timestamp not found".to_string() 
            })?;

        // Get the most recent membership for each user at that timestamp
        let query = r#"
            SELECT * FROM membership 
            WHERE room_id = $room_id 
            AND created_at <= $target_timestamp
            AND membership IN ['join', 'invite', 'knock']
            AND user_id IN (
                SELECT user_id FROM membership AS m2 
                WHERE m2.room_id = $room_id 
                AND m2.user_id = membership.user_id 
                AND m2.created_at <= $target_timestamp
                ORDER BY m2.created_at DESC 
                LIMIT 1
            )
        "#;
        
        let memberships: Vec<Membership> = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("target_timestamp", target_timestamp.to_string()))
            .await?
            .take(0)?;

        Ok(memberships)
    }

    /// Get invited users in a room
    pub async fn get_invited_users(&self, room_id: &str) -> Result<Vec<String>, RepositoryError> {
        self.get_users_by_membership_state(room_id, MembershipState::Invite).await
    }

    /// Get banned users in a room
    pub async fn get_banned_users(&self, room_id: &str) -> Result<Vec<String>, RepositoryError> {
        self.get_users_by_membership_state(room_id, MembershipState::Ban).await
    }

    /// Get users who have left a room
    pub async fn get_left_users(&self, room_id: &str) -> Result<Vec<String>, RepositoryError> {
        self.get_users_by_membership_state(room_id, MembershipState::Leave).await
    }

    /// Get users who have knocked on a room
    pub async fn get_knocked_users(&self, room_id: &str) -> Result<Vec<String>, RepositoryError> {
        self.get_users_by_membership_state(room_id, MembershipState::Knock).await
    }

    /// Accept a knock (promote knock to invite)
    pub async fn accept_knock(
        &self,
        room_id: &str,
        user_id: &str,
        accepter_id: &str,
    ) -> Result<(), RepositoryError> {
        // Check if user has knocked
        let existing_membership =
            self.get_membership(room_id, user_id).await?.ok_or_else(|| {
                RepositoryError::NotFound {
                    entity_type: "Membership".to_string(),
                    id: format!("{}:{}", room_id, user_id),
                }
            })?;

        if existing_membership.membership != MembershipState::Knock {
            return Err(RepositoryError::Validation {
                field: "membership".to_string(),
                message: "User has not knocked on this room".to_string(),
            });
        }

        // Validate accepter has permission
        if !self.is_user_in_room(room_id, accepter_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!(
                    "User {} not authorized to accept knocks in room {}",
                    accepter_id, room_id
                ),
            });
        }

        // Convert knock to invite
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Invite,
            reason: Some("Knock accepted".to_string()),
            invited_by: Some(accepter_id.to_string()),
            updated_at: Some(chrono::Utc::now()),
            display_name: existing_membership.display_name,
            avatar_url: existing_membership.avatar_url,
            is_direct: existing_membership.is_direct,
            third_party_invite: existing_membership.third_party_invite,
            join_authorised_via_users_server: existing_membership.join_authorised_via_users_server,
        };

        self.update_membership(&membership).await?;
        Ok(())
    }

    /// Reject a knock
    pub async fn reject_knock(
        &self,
        room_id: &str,
        user_id: &str,
        rejecter_id: &str,
        reason: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Check if user has knocked
        let existing_membership =
            self.get_membership(room_id, user_id).await?.ok_or_else(|| {
                RepositoryError::NotFound {
                    entity_type: "Membership".to_string(),
                    id: format!("{}:{}", room_id, user_id),
                }
            })?;

        if existing_membership.membership != MembershipState::Knock {
            return Err(RepositoryError::Validation {
                field: "membership".to_string(),
                message: "User has not knocked on this room".to_string(),
            });
        }

        // Validate rejecter has permission
        if !self.is_user_in_room(room_id, rejecter_id).await? {
            return Err(RepositoryError::Unauthorized {
                reason: format!(
                    "User {} not authorized to reject knocks in room {}",
                    rejecter_id, room_id
                ),
            });
        }

        // Convert knock to leave
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Leave,
            reason: reason.or(Some("Knock rejected".to_string())),
            invited_by: existing_membership.invited_by,
            updated_at: Some(chrono::Utc::now()),
            display_name: existing_membership.display_name,
            avatar_url: existing_membership.avatar_url,
            is_direct: existing_membership.is_direct,
            third_party_invite: existing_membership.third_party_invite,
            join_authorised_via_users_server: existing_membership.join_authorised_via_users_server,
        };

        self.update_membership(&membership).await?;
        Ok(())
    }

    /// Get membership statistics for a room
    pub async fn get_room_membership_stats(
        &self,
        room_id: &str,
    ) -> Result<std::collections::HashMap<String, u32>, RepositoryError> {
        let query = "
            SELECT membership, count() as count
            FROM membership
            WHERE room_id = $room_id
            GROUP BY membership
        ";
        let mut result = self.db.query(query).bind(("room_id", room_id.to_string())).await?;
        let stats_data: Vec<serde_json::Value> = result.take(0)?;

        let mut stats = std::collections::HashMap::new();
        for stat in stats_data {
            if let (Some(membership), Some(count)) = (
                stat.get("membership").and_then(|v| v.as_str()),
                stat.get("count").and_then(|v| v.as_u64()),
            ) {
                stats.insert(membership.to_string(), count as u32);
            }
        }

        Ok(stats)
    }

    /// Update user profile in membership
    pub async fn update_user_profile_in_room(
        &self,
        room_id: &str,
        user_id: &str,
        display_name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<(), RepositoryError> {
        let existing_membership =
            self.get_membership(room_id, user_id).await?.ok_or_else(|| {
                RepositoryError::NotFound {
                    entity_type: "Membership".to_string(),
                    id: format!("{}:{}", room_id, user_id),
                }
            })?;

        let updated_membership = Membership {
            display_name,
            avatar_url,
            updated_at: Some(chrono::Utc::now()),
            ..existing_membership
        };

        self.update_membership(&updated_membership).await?;
        Ok(())
    }

    /// Create or update membership (upsert operation)
    pub async fn upsert_membership(&self, membership: &Membership) -> Result<(), RepositoryError> {
        let membership_id = format!("{}:{}", membership.user_id, membership.room_id);
        let _: Option<Membership> = self
            .db
            .upsert(("membership", membership_id))
            .content(membership.clone())
            .await?;
        Ok(())
    }

    /// Get the count of joined members in a room
    pub async fn get_member_count(&self, room_id: &str) -> Result<i64, RepositoryError> {
        let query = "
            SELECT count() 
            FROM membership 
            WHERE room_id = $room_id 
              AND membership = 'join'
        ";

        let mut response = self.db.query(query).bind(("room_id", room_id.to_string())).await?;

        let count: Option<i64> = response.take(0)?;
        Ok(count.unwrap_or(0))
    }

    // MEMBERSHIP VALIDATION METHODS - SUBTASK 4 EXTENSIONS

    /// Validate membership state transition
    pub async fn validate_membership_transition(
        &self,
        room_id: &str,
        user_id: &str,
        old_state: MembershipState,
        new_state: MembershipState,
        sender: &str,
    ) -> Result<bool, RepositoryError> {
        // Matrix specification defines valid state transitions
        let is_valid_transition = match (old_state, new_state) {
            // From None/Leave to other states
            (MembershipState::Leave, MembershipState::Join) => {
                // Can join if room allows (would check join rules)
                true
            },
            (MembershipState::Leave, MembershipState::Invite) => {
                // Can be invited
                true
            },
            (MembershipState::Leave, MembershipState::Knock) => {
                // Can knock if room allows
                true
            },
            (MembershipState::Leave, MembershipState::Ban) => {
                // Can be banned
                true
            },

            // From Invite to other states
            (MembershipState::Invite, MembershipState::Join) => {
                // Can accept invitation (sender must be the invited user)
                sender == user_id
            },
            (MembershipState::Invite, MembershipState::Leave) => {
                // Can reject invitation or be uninvited
                sender == user_id || self.is_user_in_room(room_id, sender).await?
            },
            (MembershipState::Invite, MembershipState::Ban) => {
                // Can be banned while invited
                true
            },

            // From Join to other states
            (MembershipState::Join, MembershipState::Leave) => {
                // Can leave (self) or be kicked
                true
            },
            (MembershipState::Join, MembershipState::Ban) => {
                // Can be banned
                true
            },

            // From Knock to other states
            (MembershipState::Knock, MembershipState::Join) => {
                // Can be accepted to join
                true
            },
            (MembershipState::Knock, MembershipState::Invite) => {
                // Knock can be converted to invite
                true
            },
            (MembershipState::Knock, MembershipState::Leave) => {
                // Knock can be rejected or withdrawn
                true
            },
            (MembershipState::Knock, MembershipState::Ban) => {
                // Can be banned while knocking
                true
            },

            // From Ban to other states
            (MembershipState::Ban, MembershipState::Leave) => {
                // Can be unbanned
                true
            },

            // Same state transitions (profile updates)
            (old, new) if old == new => true,

            // All other transitions are invalid
            _ => false,
        };

        Ok(is_valid_transition)
    }

    /// Check for membership conflicts
    pub async fn check_membership_conflict(
        &self,
        room_id: &str,
        user_id: &str,
        new_membership: &MembershipContext,
    ) -> Result<Option<String>, RepositoryError> {
        // Get current membership state
        let current_membership = self.get_membership(room_id, user_id).await?;

        // Check for temporal conflicts (events with same timestamp)
        let query = "
            SELECT event_id FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.member'
            AND state_key = $user_id
            AND origin_server_ts = $timestamp
            AND event_id != $event_id
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("timestamp", new_membership.origin_server_ts))
            .bind(("event_id", new_membership.event_id.clone()))
            .await?;

        let conflicting_events: Vec<serde_json::Value> = result.take(0)?;

        if !conflicting_events.is_empty()
            && let Some(conflict) = conflicting_events.first()
            && let Some(event_id) = conflict.get("event_id").and_then(|v| v.as_str()) {
            return Ok(Some(format!("Temporal conflict with event: {}", event_id)));
        }

        // Check for state conflicts
        if let Some(current) = current_membership {
            // Check if transition is valid
            if !self
                .validate_membership_transition(
                    room_id,
                    user_id,
                    current.membership.clone(),
                    new_membership.membership.clone(),
                    &new_membership.sender,
                )
                .await?
            {
                return Ok(Some(format!(
                    "Invalid transition from {:?} to {:?}",
                    current.membership, new_membership.membership
                )));
            }
        }

        Ok(None)
    }

    /// Get current m.room.power_levels event for room as full Event object
    /// 
    /// This fetches the complete Event struct needed for state resolution,
    /// not just the content field.
    async fn get_power_levels_event_full(
        &self,
        room_id: &str,
    ) -> Result<Option<Event>, RepositoryError> {
        let query = "
            SELECT * FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.power_levels' 
            AND state_key = ''
            ORDER BY origin_server_ts DESC 
            LIMIT 1
        ";
        
        let mut result = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;
        
        let events: Vec<Event> = result.take(0)?;
        Ok(events.into_iter().next())
    }

    /// Resolve membership conflicts using Matrix State Resolution v2 algorithm
    ///
    /// Integrates with the complete StateResolver implementation to properly
    /// handle state conflicts according to the Matrix specification.
    pub async fn resolve_membership_conflict(
        &self,
        room_id: &str,
        user_id: &str,
        conflicting_events: &[String],
    ) -> Result<MembershipState, RepositoryError> {
        // Validate inputs
        if conflicting_events.is_empty() {
            return Err(RepositoryError::Validation {
                field: "conflicting_events".to_string(),
                message: "No conflicting events provided".to_string(),
            });
        }

        // Step 1: Fetch conflicting events as Event objects (not JSON)
        let mut events = Vec::new();
        for event_id in conflicting_events {
            let query = "
                SELECT * FROM event
                WHERE event_id = $event_id
            ";
            let mut result = self.db
                .query(query)
                .bind(("event_id", event_id.clone()))
                .await?;
            
            let fetched: Vec<Event> = result.take(0)?;
            if let Some(event) = fetched.into_iter().next() {
                events.push(event);
            }
        }

        if events.is_empty() {
            return Err(RepositoryError::NotFound {
                entity_type: "Conflicting events".to_string(),
                id: conflicting_events.join(", "),
            });
        }

        // Step 2: Create repository instances from same database connection
        let event_repo = Arc::new(EventRepository::new(self.db.clone()));
        let room_repo = Arc::new(RoomRepository::new(self.db.clone()));

        // Step 3: Create StateResolver instance
        let state_resolver = StateResolver::new(event_repo, room_repo);

        // Step 4: Get power levels event for conflict resolution
        let power_event = self.get_power_levels_event_full(room_id).await?;

        // Step 5: Resolve state using Matrix State Resolution v2 algorithm
        let resolved_state = state_resolver
            .resolve_state_v2(room_id, events, power_event)
            .await
            .map_err(|e| RepositoryError::StateResolution(e.to_string()))?;

        // Step 6: Extract the resolved membership event for this user
        let membership_key = ("m.room.member".to_string(), user_id.to_string());
        let resolved_event = resolved_state
            .state_events
            .get(&membership_key)
            .ok_or_else(|| {
                RepositoryError::NotFound {
                    entity_type: "Resolved membership event".to_string(),
                    id: format!("{}:{}", room_id, user_id),
                }
            })?;

        // Step 7: Extract membership state from event content
        if let Some(membership_str) = resolved_event
            .content
            .get("membership")
            .and_then(|v| v.as_str())
        {
            let membership_state = match membership_str {
                "join" => MembershipState::Join,
                "leave" => MembershipState::Leave,
                "invite" => MembershipState::Invite,
                "ban" => MembershipState::Ban,
                "knock" => MembershipState::Knock,
                _ => MembershipState::Leave,
            };
            
            tracing::info!(
                "State resolution completed for {}:{} - result: {:?}",
                room_id,
                user_id,
                membership_state
            );
            
            return Ok(membership_state);
        }

        // Default to leave if unable to parse
        Ok(MembershipState::Leave)
    }

    /// Get membership history with enhanced context
    pub async fn get_membership_history(
        &self,
        room_id: &str,
        user_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<MembershipContext>, RepositoryError> {
        let limit_clause = limit.map(|l| format!("LIMIT {}", l)).unwrap_or_default();
        let query = format!(
            "
            SELECT
                event_id, sender, content, origin_server_ts,
                auth_events, prev_events
            FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.member'
            AND state_key = $user_id
            ORDER BY origin_server_ts DESC
            {}
            ",
            limit_clause
        );

        let mut result = self
            .db
            .query(&query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        let events: Vec<serde_json::Value> = result.take(0)?;
        let mut history = Vec::new();

        for event in events {
            if let (Some(event_id), Some(sender), Some(content), Some(timestamp)) = (
                event.get("event_id").and_then(|v| v.as_str()),
                event.get("sender").and_then(|v| v.as_str()),
                event.get("content"),
                event.get("origin_server_ts").and_then(|v| v.as_i64()),
            ) {
                let membership_str =
                    content.get("membership").and_then(|v| v.as_str()).unwrap_or("leave");
                let membership = match membership_str {
                    "join" => MembershipState::Join,
                    "leave" => MembershipState::Leave,
                    "invite" => MembershipState::Invite,
                    "ban" => MembershipState::Ban,
                    "knock" => MembershipState::Knock,
                    _ => MembershipState::Leave,
                };

                let context = MembershipContext {
                    user_id: user_id.to_string(),
                    room_id: room_id.to_string(),
                    membership,
                    sender: sender.to_string(),
                    reason: content.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    invited_by: content
                        .get("invited_by")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    display_name: content
                        .get("displayname")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    avatar_url: content
                        .get("avatar_url")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    is_direct: content.get("is_direct").and_then(|v| v.as_bool()),
                    third_party_invite: content.get("third_party_invite").cloned(),
                    join_authorised_via_users_server: content
                        .get("join_authorised_via_users_server")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    event_id: event_id.to_string(),
                    origin_server_ts: timestamp,
                    auth_events: event
                        .get("auth_events")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                        })
                        .unwrap_or_default(),
                    prev_events: event
                        .get("prev_events")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                        })
                        .unwrap_or_default(),
                };

                history.push(context);
            }
        }

        Ok(history)
    }

    /// Validate a membership event
    pub async fn validate_membership_event(
        &self,
        room_id: &str,
        event: &Event,
    ) -> Result<bool, RepositoryError> {
        // Check event type
        if event.event_type != "m.room.member" {
            return Ok(false);
        }

        // Check state key exists
        let target_user = event.state_key.as_ref().ok_or_else(|| {
            RepositoryError::Validation {
                field: "state_key".to_string(),
                message: "Membership event must have state_key".to_string(),
            }
        })?;

        // Validate content structure
        let content_value = serde_json::to_value(&event.content)?;
        let membership =
            content_value.get("membership").and_then(|v| v.as_str()).ok_or_else(|| {
                RepositoryError::Validation {
                    field: "membership".to_string(),
                    message: "Membership event must have membership field".to_string(),
                }
            })?;

        // Validate membership value
        if !["join", "leave", "invite", "ban", "knock"].contains(&membership) {
            return Ok(false);
        }

        // Get current membership for validation
        let current_membership = self.get_membership(room_id, target_user).await?;
        let current_state =
            current_membership.map(|m| m.membership).unwrap_or(MembershipState::Leave);

        let new_state = match membership {
            "join" => MembershipState::Join,
            "leave" => MembershipState::Leave,
            "invite" => MembershipState::Invite,
            "ban" => MembershipState::Ban,
            "knock" => MembershipState::Knock,
            _ => return Ok(false),
        };

        // Validate transition
        self.validate_membership_transition(
            room_id,
            target_user,
            current_state,
            new_state,
            &event.sender,
        )
        .await
    }

    /// Get conflicting memberships for a user
    pub async fn get_conflicting_memberships(
        &self,
        room_id: &str,
        user_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        // Find membership events with same timestamp but different event IDs
        let query = "
            SELECT event_id, origin_server_ts
            FROM event
            WHERE room_id = $room_id
            AND event_type = 'm.room.member'
            AND state_key = $user_id
            ORDER BY origin_server_ts DESC
        ";

        let mut result = self
            .db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        let events: Vec<serde_json::Value> = result.take(0)?;
        let mut timestamp_groups: std::collections::HashMap<i64, Vec<String>> =
            std::collections::HashMap::new();

        for event in events {
            if let (Some(event_id), Some(timestamp)) = (
                event.get("event_id").and_then(|v| v.as_str()),
                event.get("origin_server_ts").and_then(|v| v.as_i64()),
            ) {
                timestamp_groups
                    .entry(timestamp)
                    .or_default()
                    .push(event_id.to_string());
            }
        }

        // Find groups with multiple events (conflicts)
        let mut conflicts = Vec::new();
        for (_, event_ids) in timestamp_groups {
            if event_ids.len() > 1 {
                conflicts.extend(event_ids);
            }
        }

        Ok(conflicts)
    }

    /// Apply membership rules and validation
    pub async fn apply_membership_rules(
        &self,
        room_id: &str,
        membership_context: &MembershipContext,
    ) -> Result<bool, RepositoryError> {
        // Check if user can perform this membership change
        let sender = &membership_context.sender;
        let target_user = &membership_context.user_id;

        // Self-membership changes
        if sender == target_user {
            match membership_context.membership {
                MembershipState::Join => {
                    // Can join if invited or if room allows
                    let current = self.get_membership(room_id, target_user).await?;
                    if let Some(membership) = current {
                        return Ok(membership.membership == MembershipState::Invite);
                    }
                    // Would check room join rules here
                    Ok(true)
                },
                MembershipState::Leave => Ok(true), // Can always leave
                MembershipState::Knock => Ok(true), // Can always knock (room rules permitting)
                _ => Ok(false),                     // Can't self-invite, self-ban
            }
        } else {
            // Other-user membership changes
            match membership_context.membership {
                MembershipState::Invite => {
                    // Check if sender can invite (would check power levels)
                    self.is_user_in_room(room_id, sender).await
                },
                MembershipState::Ban | MembershipState::Leave => {
                    // Check if sender can kick/ban (would check power levels)
                    self.is_user_in_room(room_id, sender).await
                },
                MembershipState::Join => {
                    // Others can't join on behalf of someone else
                    Ok(false)
                },
                MembershipState::Knock => {
                    // Others can't knock on behalf of someone else
                    Ok(false)
                },
            }
        }
    }

    /// Validate membership authorization chain
    pub async fn validate_membership_auth(
        &self,
        _room_id: &str,
        event: &Event,
        auth_chain: &[String],
    ) -> Result<bool, RepositoryError> {
        // Basic auth validation for membership events
        if event.event_type != "m.room.member" {
            return Ok(false);
        }

        let target_user = event.state_key.as_ref().ok_or_else(|| {
            RepositoryError::Validation {
                field: "state_key".to_string(),
                message: "Membership event must have state_key".to_string(),
            }
        })?;

        // Validate target user format
        if !target_user.contains(':') || target_user.is_empty() {
            return Err(RepositoryError::Validation {
                field: "state_key".to_string(),
                message: "Invalid user ID format in state_key".to_string(),
            });
        }

        // Check if auth chain contains required events
        let mut has_create_event = false;
        let mut has_power_levels = false;
        let mut has_join_rules = false;

        for auth_event_id in auth_chain {
            let query = "SELECT event_type FROM event WHERE event_id = $event_id";
            let mut result = self.db.query(query).bind(("event_id", auth_event_id.clone())).await?;
            let events: Vec<serde_json::Value> = result.take(0)?;

            if let Some(event) = events.first()
                && let Some(event_type) = event.get("event_type").and_then(|v| v.as_str()) {
                match event_type {
                    "m.room.create" => has_create_event = true,
                    "m.room.power_levels" => has_power_levels = true,
                    "m.room.join_rules" => has_join_rules = true,
                    _ => {},
                }
            }
        }

        // Basic requirements: must have create event
        if !has_create_event {
            return Ok(false);
        }

        // Additional validation based on membership type
        let content_value = serde_json::to_value(&event.content)?;
        if let Some(membership) = content_value.get("membership").and_then(|v| v.as_str()) {
            match membership {
                "join" => {
                    // Join events should have join rules in auth chain for validation
                    if !has_join_rules {
                        return Ok(false);
                    }
                    Ok(true)
                },
                "invite" | "ban" | "kick" => {
                    // These actions require power levels for authorization
                    if !has_power_levels {
                        return Ok(false);
                    }
                    Ok(true)
                },
                "leave" => {
                    // Leave is generally allowed
                    Ok(true)
                },
                "knock" => {
                    // Knock should have join rules in auth chain
                    if !has_join_rules {
                        return Ok(false);
                    }
                    Ok(true)
                },
                _ => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    // TASK16 SUBTASK 2: Add missing membership operation methods

    /// Kick a member from a room
    pub async fn kick_member(&self, room_id: &str, user_id: &str, _kicker_id: &str, reason: Option<&str>) -> Result<(), RepositoryError> {
        // Update membership to leave state
        let membership_id = format!("{}:{}", user_id, room_id);
        
        let query = r#"
            UPDATE membership SET 
                membership = 'leave',
                reason = $reason,
                updated_at = time::now()
            WHERE id = $membership_id
        "#;

        self.db
            .query(query)
            .bind(("membership_id", membership_id))
            .bind(("reason", reason.map(|r| r.to_string())))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "kick_member".to_string(),
            })?;

        Ok(())
    }

    /// Ban a member from a room
    pub async fn ban_member(&self, room_id: &str, user_id: &str, banner_id: &str, reason: Option<&str>) -> Result<(), RepositoryError> {
        // Update or create membership with ban state
        let membership_id = format!("{}:{}", user_id, room_id);
        
        let query = r#"
            UPSERT membership:$membership_id CONTENT {
                user_id: $user_id,
                room_id: $room_id,
                membership: 'ban',
                reason: $reason,
                invited_by: $banner_id,
                updated_at: time::now(),
                display_name: NONE,
                avatar_url: NONE,
                is_direct: false,
                third_party_invite: NONE,
                join_authorised_via_users_server: NONE
            }
        "#;

        self.db
            .query(query)
            .bind(("membership_id", membership_id))
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("reason", reason.map(|r| r.to_string())))
            .bind(("banner_id", banner_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "ban_member".to_string(),
            })?;

        Ok(())
    }

    /// Unban a member from a room
    pub async fn unban_member(&self, room_id: &str, user_id: &str, _unbanner_id: &str, reason: Option<&str>) -> Result<(), RepositoryError> {
        // Update membership from ban to leave state
        let membership_id = format!("{}:{}", user_id, room_id);
        
        let query = r#"
            UPDATE membership SET 
                membership = 'leave',
                reason = $reason,
                updated_at = time::now()
            WHERE id = $membership_id AND membership = 'ban'
        "#;

        self.db
            .query(query)
            .bind(("membership_id", membership_id))
            .bind(("reason", reason.map(|r| r.to_string())))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "unban_member".to_string(),
            })?;

        Ok(())
    }

    /// Invite a member to a room
    pub async fn invite_member(&self, room_id: &str, user_id: &str, inviter_id: &str, reason: Option<&str>) -> Result<(), RepositoryError> {
        // Create or update membership with invite state
        let membership_id = format!("{}:{}", user_id, room_id);
        
        let query = r#"
            UPSERT membership:$membership_id CONTENT {
                user_id: $user_id,
                room_id: $room_id,
                membership: 'invite',
                reason: $reason,
                invited_by: $inviter_id,
                updated_at: time::now(),
                display_name: NONE,
                avatar_url: NONE,
                is_direct: false,
                third_party_invite: NONE,
                join_authorised_via_users_server: NONE
            }
        "#;

        self.db
            .query(query)
            .bind(("membership_id", membership_id))
            .bind(("user_id", user_id.to_string()))
            .bind(("room_id", room_id.to_string()))
            .bind(("reason", reason.map(|r| r.to_string())))
            .bind(("inviter_id", inviter_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "invite_member".to_string(),
            })?;

        Ok(())
    }


    /// Forget a room as a user
    pub async fn forget_room(&self, room_id: &str, user_id: &str) -> Result<(), RepositoryError> {
        // Delete membership record
        let membership_id = format!("{}:{}", user_id, room_id);
        
        let _: Option<matryx_entity::types::Membership> = self.db
            .delete(("membership", membership_id))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "forget_room".to_string(),
            })?;

        Ok(())
    }

    /// Check if user can perform a membership action
    pub async fn can_perform_action(&self, room_id: &str, user_id: &str, action: crate::repository::room_operations::MembershipAction, target_user: Option<&str>) -> Result<bool, RepositoryError> {
        // Get user's power level
        let user_power_level = self.get_user_power_level(room_id, user_id).await.unwrap_or(0);
        
        // Get required power level for the action
        let required_power_level = match action {
            crate::repository::room_operations::MembershipAction::Kick => 50,
            crate::repository::room_operations::MembershipAction::Ban => 50,
            crate::repository::room_operations::MembershipAction::Unban => 50,
            crate::repository::room_operations::MembershipAction::Invite => 0,
            crate::repository::room_operations::MembershipAction::Join => 0,
            crate::repository::room_operations::MembershipAction::Leave => 0,
            crate::repository::room_operations::MembershipAction::Forget => 0,
        };

        // Check if user has sufficient power level
        if user_power_level < required_power_level {
            return Ok(false);
        }

        // For actions targeting other users, check target's power level
        if let Some(target_id) = target_user
            && target_id != user_id {
                let target_power_level = self.get_user_power_level(room_id, target_id).await.unwrap_or(0);
                // User must have higher power level than target
                if user_power_level <= target_power_level {
                    return Ok(false);
                }
            }

        Ok(true)
    }

    /// Get membership history for a user in a room (TASK16 version)
    pub async fn get_membership_history_events(&self, room_id: &str, user_id: &str) -> Result<Vec<crate::repository::room_operations::MembershipEvent>, RepositoryError> {
        let query = r#"
            SELECT event_id, room_id, state_key as user_id, content, sender, origin_server_ts
            FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.member' 
            AND state_key = $user_id
            ORDER BY origin_server_ts ASC
        "#;

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_membership_history_events".to_string(),
            })?;

        let events: Vec<(String, String, String, serde_json::Value, String, i64)> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_membership_history_events_parse".to_string(),
        })?;

        let membership_events = events.into_iter().map(|(event_id, room_id, user_id, content, sender, timestamp)| {
            let membership_str = content.get("membership").and_then(|m| m.as_str()).unwrap_or("leave");
            let membership = match membership_str {
                "join" => MembershipState::Join,
                "leave" => MembershipState::Leave,
                "invite" => MembershipState::Invite,
                "ban" => MembershipState::Ban,
                "knock" => MembershipState::Knock,
                _ => MembershipState::Leave,
            };

            let reason = content.get("reason").and_then(|r| r.as_str()).map(|s| s.to_string());

            crate::repository::room_operations::MembershipEvent {
                event_id,
                room_id,
                user_id,
                membership,
                reason,
                actor_id: Some(sender),
                timestamp: chrono::DateTime::from_timestamp_millis(timestamp).unwrap_or_else(chrono::Utc::now),
            }
        }).collect();

        Ok(membership_events)
    }

    /// Get user power level in a room (helper method)
    pub async fn get_user_power_level(&self, room_id: &str, user_id: &str) -> Result<i64, RepositoryError> {
        // Get power levels from room state
        let query = r#"
            SELECT content FROM event 
            WHERE room_id = $room_id 
            AND event_type = 'm.room.power_levels' 
            AND state_key = ''
            ORDER BY depth DESC, origin_server_ts DESC 
            LIMIT 1
        "#;

        let mut response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await
            .map_err(|e| RepositoryError::DatabaseError {
                message: e.to_string(),
                operation: "get_user_power_level".to_string(),
            })?;

        let content: Option<serde_json::Value> = response.take(0).map_err(|e| RepositoryError::DatabaseError {
            message: e.to_string(),
            operation: "get_user_power_level_parse".to_string(),
        })?;

        if let Some(power_levels) = content {
            if let Some(users) = power_levels.get("users").and_then(|u| u.as_object())
                && let Some(user_level) = users.get(user_id).and_then(|l| l.as_i64()) {
                    return Ok(user_level);
                }
            // Return users_default if user not explicitly listed
            if let Some(default_level) = power_levels.get("users_default").and_then(|d| d.as_i64()) {
                return Ok(default_level);
            }
        }

        Ok(0) // Default power level
    }

    /// Check if a server has users in a room (for federation access)
    pub async fn has_server_users_in_room(
        &self,
        room_id: &str,
        server_name: &str,
    ) -> Result<bool, RepositoryError> {
        let query = "
            SELECT COUNT() as count
            FROM membership
            WHERE room_id = $room_id
            AND user_id CONTAINS $server_suffix
            LIMIT 1
        ";

        let server_suffix = format!(":{}", server_name);

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("server_suffix", server_suffix))
            .await?;

        #[derive(serde::Deserialize)]
        struct CountResult {
            count: i64,
        }

        let count_result: Option<CountResult> = response.take(0)?;
        Ok(count_result.map(|c| c.count > 0).unwrap_or(false))
    }

    /// Get room membership for knock validation
    pub async fn get_room_membership_for_knock(&self, room_id: &str, user_id: &str) -> Result<Option<Membership>, RepositoryError> {
        let query = "
            SELECT * FROM membership 
            WHERE room_id = $room_id AND user_id = $user_id
            ORDER BY updated_at DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        let memberships: Vec<Membership> = response.take(0)?;
        Ok(memberships.into_iter().next())
    }

    /// Get user joined rooms for client endpoint
    pub async fn get_user_joined_rooms(&self, user_id: &str) -> Result<Vec<String>, RepositoryError> {
        let query = "
            SELECT room_id FROM membership 
            WHERE user_id = $user_id AND membership = 'join'
            ORDER BY updated_at DESC
        ";

        let mut response = self.db.query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        let room_ids: Vec<String> = response.take(0)?;
        Ok(room_ids)
    }

    /// Create knock membership for federation
    pub async fn create_knock_membership(&self, room_id: &str, user_id: &str, reason: Option<String>) -> Result<Membership, RepositoryError> {
        let membership = Membership {
            user_id: user_id.to_string(),
            room_id: room_id.to_string(),
            membership: MembershipState::Knock,
            reason,
            invited_by: None,
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        let membership_id = format!("{}:{}", room_id, user_id);
        let created_membership: Option<Membership> = self.db.create(("membership", &membership_id)).content(membership).await?;
        created_membership.ok_or_else(|| RepositoryError::NotFound { 
            entity_type: "membership".to_string(), 
            id: membership_id 
        })
    }

    /// Validate join permissions for federation
    pub async fn validate_join_permissions(
        &self,
        room_id: &str,
        user_id: &str,
        auth_service: &crate::repository::room_authorization::RoomAuthorizationService,
    ) -> Result<bool, RepositoryError> {
        // Check if user is already a member
        let existing_membership = self.get_membership(room_id, user_id).await?;
        
        match existing_membership {
            Some(membership) => {
                match membership.membership {
                    MembershipState::Join => Ok(true), // Already joined
                    MembershipState::Invite => Ok(true), // Has invite
                    MembershipState::Ban => Ok(false), // Banned
                    _ => Ok(false), // Other states don't allow join
                }
            },
            None => {
                let auth_result = auth_service
                    .validate_join_request(room_id, user_id, None)
                    .await?;
                Ok(auth_result.authorized)
            }
        }
    }

    /// Get room ban status for unban operations
    pub async fn get_room_ban_status(&self, room_id: &str, user_id: &str) -> Result<Option<Membership>, RepositoryError> {
        let query = "
            SELECT * FROM membership 
            WHERE room_id = $room_id AND user_id = $user_id AND membership = 'ban'
            ORDER BY updated_at DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        let memberships: Vec<Membership> = response.take(0)?;
        Ok(memberships.into_iter().next())
    }

    /// Validate unban permissions
    pub async fn validate_unban_permissions(
        &self,
        room_id: &str,
        unbanner_user_id: &str,
        target_user_id: &str,
        auth_service: &crate::repository::room_authorization::RoomAuthorizationService,
    ) -> Result<bool, RepositoryError> {
        // Check if target user is actually banned
        let ban_status = self.get_room_ban_status(room_id, target_user_id).await?;
        if ban_status.is_none() {
            return Ok(false); // User is not banned
        }

        // Validate using authorization service
        let auth_result = auth_service
            .validate_room_operation(
                room_id,
                unbanner_user_id,
                "unban",
                Some(target_user_id)
            )
            .await?;

        Ok(auth_result.authorized)
    }

    /// Update membership for unban operation
    pub async fn update_unban_membership(&self, room_id: &str, user_id: &str) -> Result<(), RepositoryError> {
        let query = "
            UPDATE membership SET 
                membership = 'leave',
                updated_at = $updated_at
            WHERE room_id = $room_id AND user_id = $user_id AND membership = 'ban'
        ";

        self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .bind(("updated_at", chrono::Utc::now()))
            .await?;

        Ok(())
    }

    /// Check if a server has users in a room for federation permission checks
    pub async fn check_server_has_users_in_room(&self, room_id: &str, server_name: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT COUNT() as count
            FROM membership
            WHERE room_id = $room_id
            AND user_id CONTAINS $server_suffix
            AND membership IN ['join', 'invite', 'leave']
            LIMIT 1
        ";

        let server_suffix = format!(":{}", server_name);

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("server_suffix", server_suffix))
            .await?;

        #[derive(serde::Deserialize)]
        struct CountResult {
            count: i64,
        }

        let count_result: Option<CountResult> = response.take(0)?;
        Ok(count_result.map(|c| c.count > 0).unwrap_or(false))
    }

    /// Upsert membership record for unban operations
    pub async fn upsert_membership_record(&self, membership: Membership) -> Result<(), RepositoryError> {
        let membership_id = format!("{}:{}", membership.user_id, membership.room_id);
        
        let _: Option<Membership> = self.db
            .upsert(("membership", membership_id))
            .content(membership)
            .await?;

        Ok(())
    }

    /// Check user's membership status in a room
    pub async fn get_user_membership_status(&self, room_id: &str, user_id: &str) -> Result<Option<String>, RepositoryError> {
        let query = "
            SELECT membership
            FROM membership
            WHERE room_id = $room_id AND user_id = $user_id
            ORDER BY updated_at DESC
            LIMIT 1
        ";

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("user_id", user_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct MembershipResult {
            membership: String,
        }

        let membership: Option<MembershipResult> = response.take(0)?;
        Ok(membership.map(|m| m.membership))
    }

    /// Find a user from a specific server who can authorize restricted room joins
    pub async fn find_authorizing_user(&self, room_id: &str, server_name: &str) -> Result<Option<String>, RepositoryError> {
        let query = "
            SELECT user_id
            FROM membership
            WHERE room_id = $room_id
            AND membership = 'join'
            AND user_id CONTAINS $server_suffix
            ORDER BY updated_at DESC
            LIMIT 1
        ";

        let server_suffix = format!(":{}", server_name);

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("server_suffix", server_suffix))
            .await?;

        #[derive(serde::Deserialize)]
        struct AuthUser {
            user_id: String,
        }

        let auth_user: Option<AuthUser> = response.take(0)?;
        Ok(auth_user.map(|u| u.user_id))
    }

    /// Check if a server has any users in a room (current or historical)
    pub async fn server_has_users_in_room(&self, room_id: &str, server_name: &str) -> Result<bool, RepositoryError> {
        let query = "
            SELECT COUNT() as count
            FROM membership
            WHERE room_id = $room_id
            AND user_id CONTAINS $server_suffix
            LIMIT 1
        ";

        let server_suffix = format!(":{}", server_name);

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("server_suffix", server_suffix))
            .await?;

        #[derive(serde::Deserialize)]
        struct CountResult {
            count: i64,
        }

        let count_result: Option<CountResult> = response.take(0)?;
        Ok(count_result.map(|c| c.count > 0).unwrap_or(false))
    }

    /// Get all joined rooms for a user
    pub async fn get_joined_rooms_for_user(&self, user_id: &str) -> Result<Vec<String>, RepositoryError> {
        let query = "
            SELECT room_id 
            FROM membership 
            WHERE user_id = $user_id 
              AND membership = 'join'
            ORDER BY updated_at DESC
        ";

        let mut response = self.db.query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        #[derive(serde::Deserialize)]
        struct RoomResult {
            room_id: String,
        }

        let room_results: Vec<RoomResult> = response.take(0)?;
        Ok(room_results.into_iter().map(|r| r.room_id).collect())
    }

    /// Create a live query for membership changes for a specific user
    pub async fn create_user_membership_live_query(
        &self,
        user_id: &str,
    ) -> Result<surrealdb::Response, RepositoryError> {
        let query = r#"
            LIVE SELECT *, meta::id(id) as membership_id FROM membership
            WHERE room_id IN (
                SELECT VALUE room_id FROM membership 
                WHERE user_id = $user_id AND membership IN ['join', 'invite']
            )
        "#;

        let response = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        Ok(response)
    }

    /// Create a live query for membership changes in a specific room
    pub async fn create_room_membership_live_query(
        &self,
        room_id: &str,
    ) -> Result<surrealdb::Response, RepositoryError> {
        let query = r#"
            LIVE SELECT *, meta::id(id) as membership_id FROM membership
            WHERE room_id = $room_id
        "#;

        let response = self.db
            .query(query)
            .bind(("room_id", room_id.to_string()))
            .await?;

        Ok(response)
    }

    /// Create a live query for membership changes in multiple rooms
    pub async fn create_batched_membership_live_query(
        &self,
        room_ids: Vec<String>,
    ) -> Result<surrealdb::Response, RepositoryError> {
        let query = r#"
            LIVE SELECT *, meta::id(id) as membership_id FROM membership
            WHERE room_id IN $room_ids
        "#;

        let response = self.db
            .query(query)
            .bind(("room_ids", room_ids))
            .await?;

        Ok(response)
    }

    /// Create a simple live query for membership changes for a specific user
    pub async fn create_user_membership_simple_live_query(
        &self,
        user_id: &str,
    ) -> Result<surrealdb::Response, RepositoryError> {
        let query = "LIVE SELECT * FROM membership WHERE user_id = $user_id";

        let response = self.db
            .query(query)
            .bind(("user_id", user_id.to_string()))
            .await?;

        Ok(response)
    }

    /// Get list of remote server names that have users in this room
    pub async fn get_remote_servers_in_room(
        &self,
        room_id: &str,
    ) -> Result<Vec<String>, RepositoryError> {
        let memberships: Vec<Membership> = self.db
            .query("SELECT * FROM membership WHERE room_id = $room_id AND membership = 'join'")
            .bind(("room_id", room_id.to_string()))
            .await?
            .take(0)?;

        // Extract server names from user IDs
        let servers: std::collections::HashSet<String> = memberships
            .iter()
            .filter_map(|m| {
                // user_id format: @localpart:server.name
                m.user_id.split(':').nth(1).map(|s| s.to_string())
            })
            .collect();

        Ok(servers.into_iter().collect())
    }

    /// Check if a server had users in the room at a specific event depth
    /// 
    /// This provides more precise access control than checking current membership,
    /// ensuring servers only see events from when they actually had members present.
    /// 
    /// # Arguments
    /// * `room_id` - The room to check
    /// * `server_name` - The server to check membership for
    /// * `depth` - The event depth to check membership at
    /// 
    /// # Returns
    /// True if the server had joined or invited users at the specified depth
    pub async fn get_server_membership_at_depth(
        &self,
        room_id: &str,
        server_name: &str,
        depth: i64,
    ) -> Result<bool, RepositoryError> {
        // Query for membership events from this server up to the specified depth
        let query = "
            SELECT COUNT() as count
            FROM membership m
            JOIN event e ON e.room_id = m.room_id 
                AND e.event_type = 'm.room.member' 
                AND e.state_key = m.user_id
            WHERE m.room_id = $room_id
            AND m.user_id CONTAINS $server_suffix
            AND m.membership IN ['join', 'invite']
            AND e.depth <= $depth
            LIMIT 1
        ";

        let server_suffix = format!(":{}", server_name);

        let mut response = self.db.query(query)
            .bind(("room_id", room_id.to_string()))
            .bind(("server_suffix", server_suffix))
            .bind(("depth", depth))
            .await?;

        #[derive(serde::Deserialize)]
        struct CountResult {
            count: i64,
        }

        let count_result: Option<CountResult> = response.take(0)?;
        Ok(count_result.map(|c| c.count > 0).unwrap_or(false))
    }
}
