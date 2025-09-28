use crate::repository::error::RepositoryError;
use crate::repository::sync::{SyncRepository, SyncResponse, RoomSyncData, JoinedRoomSync, InvitedRoomSync, StateSync, AccountDataSync, InviteStateSync};
use crate::repository::presence::{PresenceRepository, PresenceEvent};
use crate::repository::room::RoomRepository;
use crate::repository::membership::MembershipRepository;
use matryx_entity::types::{MatrixFilter, MembershipState};

use std::sync::Arc;

// TASK14 SUBTASK 7: Create coordinated sync service
pub struct SyncService {
    sync_repo: Arc<SyncRepository>,
    presence_repo: Arc<PresenceRepository>,
    room_repo: Arc<RoomRepository>,
    membership_repo: Arc<MembershipRepository>,
}

impl SyncService {
    pub fn new(
        sync_repo: Arc<SyncRepository>,
        presence_repo: Arc<PresenceRepository>,
        room_repo: Arc<RoomRepository>,
        membership_repo: Arc<MembershipRepository>,
    ) -> Self {
        Self {
            sync_repo,
            presence_repo,
            room_repo,
            membership_repo,
        }
    }

    /// Get full sync response for initial sync
    pub async fn get_full_sync_response(&self, user_id: &str, filter: Option<&MatrixFilter>) -> Result<SyncResponse, RepositoryError> {
        // Get initial sync data using SyncRepository
        let filter_converted = filter.map(|f| self.convert_matrix_filter_to_sync_filter(f));
        let sync_response = self.sync_repo.get_initial_sync_data(user_id, filter_converted.as_ref()).await?;

        // Get joined rooms for the user using MembershipRepository
        let user_memberships = self.membership_repo.get_user_rooms(user_id).await?;
        let mut join_rooms = std::collections::HashMap::new();

        // Populate room sync data for joined rooms
        for membership in &user_memberships {
            if matches!(membership.membership, MembershipState::Join) {
                let room_sync_data = self.sync_repo.get_room_sync_data(user_id, &membership.room_id, None).await?;
                let joined_room = JoinedRoomSync {
                    state: StateSync { events: room_sync_data.state },
                    timeline: room_sync_data.timeline,
                    ephemeral: room_sync_data.ephemeral,
                    account_data: AccountDataSync { events: room_sync_data.account_data },
                    unread_notifications: room_sync_data.unread_notifications,
                    summary: room_sync_data.summary,
                };
                join_rooms.insert(membership.room_id.clone(), joined_room);
            }
        }

        // Get invited rooms from the same membership data
        let mut invite_rooms = std::collections::HashMap::new();

        // Populate invite sync data for invited rooms
        for membership in &user_memberships {
            if matches!(membership.membership, MembershipState::Invite) {
                let room_sync_data = self.sync_repo.get_room_sync_data(user_id, &membership.room_id, None).await?;
                let invited_room = InvitedRoomSync {
                    invite_state: InviteStateSync { events: room_sync_data.state },
                };
                invite_rooms.insert(membership.room_id.clone(), invited_room);
            }
        }

        // Convert to SyncResponse format
        Ok(SyncResponse {
            next_batch: sync_response.next_batch,
            rooms: crate::repository::sync::RoomsSyncData {
                join: join_rooms,
                invite: invite_rooms,
                leave: std::collections::HashMap::new(),
                knock: std::collections::HashMap::new(),
            },
            presence: Some(sync_response.presence),
            account_data: Some(sync_response.account_data),
            to_device: None,
            device_lists: None,
            device_one_time_keys_count: None,
            device_unused_fallback_key_types: None,
        })
    }

    /// Get incremental sync response for subsequent syncs
    pub async fn get_incremental_sync_response(&self, user_id: &str, since: &str, filter: Option<&MatrixFilter>) -> Result<SyncResponse, RepositoryError> {
        let filter_converted = filter.map(|f| self.convert_matrix_filter_to_sync_filter(f));
        self.sync_repo.get_incremental_sync_data(user_id, since, filter_converted.as_ref()).await
    }

    /// Get room sync data for a specific room
    pub async fn get_room_sync_data(&self, user_id: &str, room_id: &str, since: Option<&str>) -> Result<RoomSyncData, RepositoryError> {
        // Verify user has access to this room using RoomRepository
        let room_info = self.room_repo.get_room_public_info(room_id).await?;

        // Check if room is public or user is a member
        if let Some(room_info) = room_info
            && !room_info.world_readable {
                // For private rooms, verify membership
                let membership = self.membership_repo.get_membership(user_id, room_id).await?;
                if membership.is_none() {
                    return Err(RepositoryError::NotFound {
                        entity_type: "Membership".to_string(),
                        id: format!("{}:{}", user_id, room_id)
                    });
                }
            }

        self.sync_repo.get_room_sync_data(user_id, room_id, since).await
    }

    /// Apply sync filter to sync response
    pub async fn apply_sync_filter(&self, sync_data: SyncResponse, filter: &MatrixFilter) -> Result<SyncResponse, RepositoryError> {
        // Apply filter logic based on Matrix filter specification
        let mut filtered_response = sync_data;

        // Apply room filter if specified
        if let Some(room_filter) = &filter.room {
            if let Some(not_rooms) = &room_filter.not_rooms {
                // Remove rooms that are in the not_rooms list
                filtered_response.rooms.join.retain(|room_id, _| !not_rooms.contains(room_id));
                filtered_response.rooms.invite.retain(|room_id, _| !not_rooms.contains(room_id));
                filtered_response.rooms.leave.retain(|room_id, _| !not_rooms.contains(room_id));
                filtered_response.rooms.knock.retain(|room_id, _| !not_rooms.contains(room_id));
            }

            if let Some(rooms) = &room_filter.rooms
                && !rooms.is_empty() {
                    // Only include rooms that are in the rooms list
                    filtered_response.rooms.join.retain(|room_id, _| rooms.contains(room_id));
                    filtered_response.rooms.invite.retain(|room_id, _| rooms.contains(room_id));
                    filtered_response.rooms.leave.retain(|room_id, _| rooms.contains(room_id));
                    filtered_response.rooms.knock.retain(|room_id, _| rooms.contains(room_id));
                }
        }

        // Apply presence filter if specified
        if let Some(presence_filter) = &filter.presence {
            if let Some(not_senders) = &presence_filter.not_senders
                && let Some(ref mut presence) = filtered_response.presence {
                    presence.events.retain(|event| {
                        if let Some(sender) = event.get("sender").and_then(|s| s.as_str()) {
                            !not_senders.iter().any(|s| s == sender)
                        } else {
                            false
                        }
                    });
                }

            if let Some(senders) = &presence_filter.senders
                && !senders.is_empty()
                && let Some(ref mut presence) = filtered_response.presence {
                    presence.events.retain(|event| {
                        if let Some(sender) = event.get("sender").and_then(|s| s.as_str()) {
                            senders.iter().any(|s| s == sender)
                        } else {
                            false
                        }
                    });
                }
        }

        // Apply account data filter if specified
        if let Some(account_data_filter) = &filter.account_data {
            if let Some(not_types) = &account_data_filter.not_types
                && let Some(ref mut account_data) = filtered_response.account_data {
                    account_data.events.retain(|event| {
                        if let Some(event_type) = event.get("type").and_then(|t| t.as_str()) {
                            !not_types.iter().any(|t| t == event_type)
                        } else {
                            false
                        }
                    });
                }

            if let Some(types) = &account_data_filter.types
                && !types.is_empty()
                && let Some(ref mut account_data) = filtered_response.account_data {
                    account_data.events.retain(|event| {
                        if let Some(event_type) = event.get("type").and_then(|t| t.as_str()) {
                            types.iter().any(|t| t == event_type)
                        } else {
                            false
                        }
                    });
                }
        }

        Ok(filtered_response)
    }

    /// Get presence sync data for a user
    pub async fn get_presence_sync_data(&self, user_id: &str, since: Option<&str>) -> Result<Vec<PresenceEvent>, RepositoryError> {
        let since_time = if let Some(since_token) = since {
            let since_position = self.sync_repo.parse_sync_token(since_token).await?;
            Some(since_position.timestamp)
        } else {
            None
        };

        self.presence_repo.get_user_presence_events(user_id, since_time).await
    }

    // Helper method to convert MatrixFilter to sync Filter
    fn convert_matrix_filter_to_sync_filter(&self, _matrix_filter: &MatrixFilter) -> crate::repository::sync::Filter {
        // Simplified conversion - in real implementation would properly convert all filter fields
        crate::repository::sync::Filter {
            room: None,
            presence: None,
            account_data: None,
            event_format: None,
            event_fields: None,
        }
    }
}