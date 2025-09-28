use crate::repository::error::RepositoryError;
use crate::repository::public_rooms::{PublicRoomsRepository, PublicRoomsResponse, PublicRoomsFilter, RoomDirectoryVisibility, PublicRoomEntry};
use crate::repository::room::RoomRepository;
use crate::repository::room_alias::RoomAliasRepository;
use crate::repository::membership::MembershipRepository;
use std::sync::Arc;

// TASK15 SUBTASK 8: Create coordinated room discovery service
pub struct RoomDiscoveryService {
    public_rooms_repo: Arc<PublicRoomsRepository>,
    room_repo: Arc<RoomRepository>,
    room_alias_repo: Arc<RoomAliasRepository>,
    membership_repo: Arc<MembershipRepository>,
}

impl RoomDiscoveryService {
    pub fn new(
        public_rooms_repo: Arc<PublicRoomsRepository>,
        room_repo: Arc<RoomRepository>,
        room_alias_repo: Arc<RoomAliasRepository>,
        membership_repo: Arc<MembershipRepository>,
    ) -> Self {
        Self {
            public_rooms_repo,
            room_repo,
            room_alias_repo,
            membership_repo,
        }
    }

    /// Get public rooms list with filtering
    pub async fn get_public_rooms_list(&self, filter: PublicRoomsFilter) -> Result<PublicRoomsResponse, RepositoryError> {
        if let Some(search_term) = filter.server.as_deref() {
            // If server filter is specified, get federation public rooms
            self.public_rooms_repo.get_federation_public_rooms(search_term, filter.limit).await
        } else {
            // Regular public rooms listing
            self.public_rooms_repo.get_public_rooms(filter.limit, filter.since.as_deref()).await
        }
    }

    /// Search rooms with query and filter
    pub async fn search_rooms(&self, query: &str, filter: PublicRoomsFilter) -> Result<PublicRoomsResponse, RepositoryError> {
        if query.trim().is_empty() {
            // If no search query, return regular public rooms list
            self.get_public_rooms_list(filter).await
        } else {
            // Search public rooms with the query
            self.public_rooms_repo.search_public_rooms(query, filter.limit).await
        }
    }

    /// Set room directory visibility
    pub async fn set_room_directory_visibility(&self, room_id: &str, visibility: RoomDirectoryVisibility, requesting_user: &str) -> Result<(), RepositoryError> {
        // Check if user has permission to modify room directory visibility
        let membership = self.membership_repo.get_membership(room_id, requesting_user).await?;
        
        match membership {
            Some(membership) if membership.membership == matryx_entity::types::MembershipState::Join => {
                // Check if user has sufficient power level (simplified - in real implementation would check power levels)
                self.public_rooms_repo.add_room_to_directory(room_id, visibility).await
            },
            _ => Err(RepositoryError::ValidationError {
                field: "user_id".to_string(),
                message: "User not authorized to modify room directory visibility".to_string(),
            }),
        }
    }

    /// Get room directory entry
    pub async fn get_room_directory_entry(&self, room_id: &str) -> Result<Option<PublicRoomEntry>, RepositoryError> {
        let public_info = self.room_repo.get_room_public_info(room_id).await?;
        
        if let Some(info) = public_info {
            match info.visibility {
                RoomDirectoryVisibility::Public => {
                    Ok(Some(PublicRoomEntry {
                        room_id: info.room_id,
                        name: info.name,
                        topic: info.topic,
                        canonical_alias: info.canonical_alias,
                        num_joined_members: info.num_joined_members,
                        avatar_url: info.avatar_url,
                        world_readable: info.world_readable,
                        guest_can_join: info.guest_can_join,
                        join_rule: info.join_rule,
                        room_type: info.room_type,
                    }))
                },
                RoomDirectoryVisibility::Private => Ok(None),
            }
        } else {
            Ok(None)
        }
    }

    /// Update room directory statistics
    pub async fn update_room_directory_stats(&self, room_id: &str) -> Result<(), RepositoryError> {
        // Get current member count
        let member_count = self.room_repo.get_room_member_count(room_id).await?;
        
        // Get current room info
        if let Some(mut room_info) = self.room_repo.get_room_public_info(room_id).await? {
            // Update member count
            room_info.num_joined_members = member_count;
            
            // Update the room info
            self.room_repo.update_room_public_info(room_id, &room_info).await?;
        }
        
        Ok(())
    }

    /// Get federation public rooms for a specific server
    pub async fn get_federation_public_rooms(&self, server_name: &str, filter: PublicRoomsFilter) -> Result<PublicRoomsResponse, RepositoryError> {
        self.public_rooms_repo.get_federation_public_rooms(server_name, filter.limit).await
    }

    /// Resolve room alias to room ID and get public info
    pub async fn resolve_alias_and_get_public_info(&self, alias: &str) -> Result<Option<PublicRoomEntry>, RepositoryError> {
        // Resolve alias to room ID
        if let Some(alias_info) = self.room_alias_repo.resolve_alias(alias).await? {
            // Get public room entry
            self.get_room_directory_entry(&alias_info.room_id).await
        } else {
            Ok(None)
        }
    }

    /// Get room statistics for directory
    pub async fn get_room_statistics(&self, room_id: &str) -> Result<RoomStatistics, RepositoryError> {
        let member_count = self.room_repo.get_room_member_count(room_id).await?;
        let public_info = self.room_repo.get_room_public_info(room_id).await?;
        
        let visibility = public_info.as_ref()
            .map(|info| info.visibility.clone())
            .unwrap_or(RoomDirectoryVisibility::Private);
        
        Ok(RoomStatistics {
            room_id: room_id.to_string(),
            member_count,
            visibility,
            last_activity: None, // Could be implemented to track last message timestamp
        })
    }

    /// Update room search index (placeholder for full-text search implementation)
    pub async fn update_room_search_index(&self, room_id: &str) -> Result<(), RepositoryError> {
        // In a full implementation, this would update search indexes
        // For now, we'll just ensure the room directory info is up to date
        self.update_room_directory_stats(room_id).await
    }
}

// Supporting types for room discovery service
#[derive(Debug, Clone)]
pub struct RoomStatistics {
    pub room_id: String,
    pub member_count: u32,
    pub visibility: RoomDirectoryVisibility,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}