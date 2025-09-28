use crate::repository::error::RepositoryError;
use surrealdb::{Connection, Surreal};

// Import core repositories
use crate::repository::{
    UserRepository, RoomRepository, EventRepository, MembershipRepository,
    DeviceRepository, CrossSigningRepository, ThirdPartyService,
    PublicRoomsRepository, CryptoRepository, MediaRepository
};

/// Core Matrix service that coordinates essential repositories
/// This provides a single entry point for core Matrix operations
pub struct MatrixService<C: Connection> {
    // Core repositories
    pub user_repo: UserRepository,
    pub room_repo: RoomRepository,
    pub event_repo: EventRepository,
    pub membership_repo: MembershipRepository,
    
    // Device and crypto repositories
    pub device_repo: DeviceRepository,
    pub cross_signing_repo: CrossSigningRepository,
    
    // Third-party integration
    pub third_party_service: ThirdPartyService<C>,
    
    // Public rooms
    pub public_rooms_repo: PublicRoomsRepository,
}

impl<C: Connection> MatrixService<C> {
    /// Create a new MatrixService with core repositories initialized
    pub fn new(db: Surreal<C>) -> Self {
        Self {
            // Core repositories
            user_repo: UserRepository::new(db.clone()),
            room_repo: RoomRepository::new(db.clone()),
            event_repo: EventRepository::new(db.clone()),
            membership_repo: MembershipRepository::new(db.clone()),
            
            // Device and crypto repositories
            device_repo: DeviceRepository::new(db.clone()),
            cross_signing_repo: CrossSigningRepository::new(db.clone()),
            
            // Third-party integration
            third_party_service: ThirdPartyService::new(db.clone()),
            
            // Public rooms
            public_rooms_repo: PublicRoomsRepository::new(db.clone()),
        }
    }

    /// Get a reference to the database connection
    pub fn db(&self) -> &Surreal<C> {
        self.third_party_service.third_party_repo().db()
    }

    /// Perform health check on all critical repositories
    pub async fn health_check(&self) -> Result<HealthStatus, RepositoryError> {
        let mut status = HealthStatus {
            overall: HealthState::Healthy,
            components: std::collections::HashMap::new(),
        };

        // Check core repositories
        status.components.insert("user_repo".to_string(), self.check_user_repo_health().await);
        status.components.insert("room_repo".to_string(), self.check_room_repo_health().await);
        status.components.insert("event_repo".to_string(), self.check_event_repo_health().await);
        status.components.insert("membership_repo".to_string(), self.check_membership_repo_health().await);

        // Check if any components are unhealthy
        for (_, component_status) in &status.components {
            if *component_status != HealthState::Healthy {
                status.overall = HealthState::Degraded;
                break;
            }
        }

        Ok(status)
    }

    /// Check user repository health
    async fn check_user_repo_health(&self) -> HealthState {
        // Simple health check - try to query user count
        match self.user_repo.get_all_users(Some(1)).await {
            Ok(_) => HealthState::Healthy,
            Err(_) => HealthState::Unhealthy,
        }
    }

    /// Check room repository health
    async fn check_room_repo_health(&self) -> HealthState {
        // Simple health check - try to get room count
        match self.room_repo.get_room_count().await {
            Ok(_) => HealthState::Healthy,
            Err(_) => HealthState::Unhealthy,
        }
    }

    /// Check event repository health
    async fn check_event_repo_health(&self) -> HealthState {
        // Simple health check - try to get recent events
        match self.event_repo.get_recent_events(1).await {
            Ok(_) => HealthState::Healthy,
            Err(_) => HealthState::Unhealthy,
        }
    }

    /// Check membership repository health
    async fn check_membership_repo_health(&self) -> HealthState {
        // Simple health check - try to get membership stats
        match self.membership_repo.get_room_membership_stats("!test:example.com").await {
            Ok(_) => HealthState::Healthy,
            Err(_) => HealthState::Degraded, // Non-critical if test room doesn't exist
        }
    }

    /// Get service statistics
    pub async fn get_service_statistics(&self) -> Result<ServiceStatistics, RepositoryError> {
        let user_count = self.user_repo.get_all_users(None).await?.len() as u64;
        let room_count = self.room_repo.get_room_count().await?;
        let event_count = self.event_repo.get_total_event_count().await?;

        Ok(ServiceStatistics {
            user_count,
            room_count,
            event_count,
            uptime_seconds: 0, // Would be calculated from service start time
        })
    }

    /// Perform coordinated user creation across multiple repositories
    pub async fn create_user_with_profile(
        &self,
        user_id: &str,
        password_hash: &str,
        display_name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<(), RepositoryError> {
        // Create user in user repository
        let user = matryx_entity::types::User {
            user_id: user_id.to_string(),
            password_hash: password_hash.to_string(),
            display_name: display_name.clone(),
            avatar_url: avatar_url.clone(),
            is_active: true,
            is_admin: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        self.user_repo.create(&user).await?;

        // User profile is stored in the user record itself

        // Additional initialization can be added here as needed

        Ok(())
    }

    /// Perform coordinated room creation across multiple repositories
    pub async fn create_room_with_state(
        &self,
        room_id: &str,
        creator_user_id: &str,
        room_name: Option<String>,
        room_topic: Option<String>,
        is_public: bool,
    ) -> Result<(), RepositoryError> {
        // Create room in room repository
        let room = matryx_entity::types::Room {
            room_id: room_id.to_string(),
            room_version: "10".to_string(),
            creator: creator_user_id.to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        self.room_repo.create(&room).await?;

        // Add creator as member
        let membership = matryx_entity::types::Membership {
            user_id: creator_user_id.to_string(),
            room_id: room_id.to_string(),
            membership: matryx_entity::types::MembershipState::Join,
            reason: None,
            invited_by: None,
            updated_at: Some(chrono::Utc::now()),
            display_name: None,
            avatar_url: None,
            is_direct: None,
            third_party_invite: None,
            join_authorised_via_users_server: None,
        };

        self.membership_repo.create_membership(&membership).await?;

        // Create initial room state events
        if let Some(name) = room_name {
            self.event_repo.create_room_name_event(room_id, creator_user_id, &name).await?;
        }

        if let Some(topic) = room_topic {
            self.event_repo.create_room_topic_event(room_id, creator_user_id, &topic).await?;
        }

        // Set room visibility
        if is_public {
            self.public_rooms_repo.add_room_to_directory(room_id).await?;
        }

        Ok(())
    }
}

/// Health status for the Matrix service
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub overall: HealthState,
    pub components: std::collections::HashMap<String, HealthState>,
}

/// Health state enumeration
#[derive(Debug, Clone, PartialEq)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Service statistics
#[derive(Debug, Clone)]
pub struct ServiceStatistics {
    pub user_count: u64,
    pub room_count: u64,
    pub event_count: u64,
    pub uptime_seconds: u64,
}