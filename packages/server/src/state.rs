use crate::auth::MatrixSessionService;
use crate::cache::lazy_loading_cache::LazyLoadingCache;
use crate::config::ServerConfig;
use crate::federation::event_signer::EventSigner;
use crate::metrics::lazy_loading_metrics::LazyLoadingMetrics;
use crate::monitoring::memory_tracker::LazyLoadingMemoryTracker;
use matryx_surrealdb::repository::{
    MentionRepository,
    ServerNoticeRepository,
    ThreadRepository,
    event::EventRepository,
    membership::MembershipRepository,
    monitoring::MonitoringRepository,
    performance::PerformanceRepository,
    relations::RelationsRepository,
    room::RoomRepository,
    room_operations::RoomOperationsService,
    threads::ThreadsRepository,
};
use std::sync::Arc;
use surrealdb::{Surreal, engine::any::Any};

#[derive(Clone)]
pub struct AppState {
    pub db: Surreal<Any>,
    pub session_service: Arc<MatrixSessionService<Any>>,
    pub homeserver_name: String,
    pub config: &'static ServerConfig,
    pub http_client: Arc<reqwest::Client>,
    pub event_signer: Arc<EventSigner>,
    pub thread_repository: Arc<ThreadRepository<Any>>,
    pub mention_repository: Arc<MentionRepository>,
    pub server_notice_repository: Arc<ServerNoticeRepository<Any>>,
    /// Room operations service that coordinates between room-related repositories
    pub room_operations: Arc<RoomOperationsService<Any>>,
    /// Enhanced lazy loading cache with SurrealDB LiveQuery integration
    pub lazy_loading_cache: Option<Arc<LazyLoadingCache>>,
    /// Performance metrics for lazy loading monitoring
    pub lazy_loading_metrics: Option<Arc<LazyLoadingMetrics>>,
    /// Memory usage tracker for cache lifecycle management
    pub memory_tracker: Option<Arc<LazyLoadingMemoryTracker>>,
}

impl AppState {
    pub fn new(
        db: Surreal<Any>,
        session_service: Arc<MatrixSessionService<Any>>,
        homeserver_name: String,
        config: &'static ServerConfig,
        http_client: Arc<reqwest::Client>,
        event_signer: Arc<EventSigner>,
    ) -> Self {
        let thread_repository = Arc::new(ThreadRepository::new(db.clone()));
        let mention_repository = Arc::new(MentionRepository::new(db.clone()));
        let server_notice_repository = Arc::new(ServerNoticeRepository::new(db.clone()));

        // Initialize repositories for room operations service
        let room_repo = RoomRepository::new(db.clone());
        let event_repo = EventRepository::new(db.clone());
        let membership_repo = MembershipRepository::new(db.clone());
        let relations_repo = RelationsRepository::new(db.clone());
        let threads_repo = ThreadsRepository::new(db.clone());

        // Create room operations service
        let room_operations = Arc::new(RoomOperationsService::new(
            room_repo,
            event_repo,
            membership_repo,
            relations_repo,
            threads_repo,
        ));

        Self {
            db,
            session_service,
            homeserver_name,
            config,
            http_client,
            event_signer,
            thread_repository,
            mention_repository,
            server_notice_repository,
            room_operations,
            lazy_loading_cache: None,
            lazy_loading_metrics: None,
            memory_tracker: None,
        }
    }

    /// Create AppState with enhanced lazy loading optimization enabled
    pub fn with_lazy_loading_optimization(
        db: Surreal<Any>,
        session_service: Arc<MatrixSessionService<Any>>,
        homeserver_name: String,
        config: &'static ServerConfig,
        http_client: Arc<reqwest::Client>,
        event_signer: Arc<EventSigner>,
    ) -> Self {
        // Initialize lazy loading components
        let lazy_cache = Arc::new(LazyLoadingCache::new());

        // Create repositories for metrics and monitoring
        let performance_repo = Arc::new(PerformanceRepository::new(db.clone()));
        let monitoring_repo = Arc::new(MonitoringRepository::new(db.clone()));

        let metrics = Arc::new(LazyLoadingMetrics::new(performance_repo.clone()));
        let memory_tracker =
            Arc::new(LazyLoadingMemoryTracker::new(performance_repo, monitoring_repo));

        // Set baseline memory usage
        memory_tracker.set_baseline(std::mem::size_of::<LazyLoadingCache>());

        let thread_repository = Arc::new(ThreadRepository::new(db.clone()));
        let mention_repository = Arc::new(MentionRepository::new(db.clone()));
        let server_notice_repository = Arc::new(ServerNoticeRepository::new(db.clone()));

        // Initialize repositories for room operations service
        let room_repo = RoomRepository::new(db.clone());
        let event_repo = EventRepository::new(db.clone());
        let membership_repo = MembershipRepository::new(db.clone());
        let relations_repo = RelationsRepository::new(db.clone());
        let threads_repo = ThreadsRepository::new(db.clone());

        // Create room operations service
        let room_operations = Arc::new(RoomOperationsService::new(
            room_repo,
            event_repo,
            membership_repo,
            relations_repo,
            threads_repo,
        ));

        Self {
            db,
            session_service,
            homeserver_name,
            config,
            http_client,
            event_signer,
            thread_repository,
            mention_repository,
            server_notice_repository,
            room_operations,
            lazy_loading_cache: Some(lazy_cache),
            lazy_loading_metrics: Some(metrics),
            memory_tracker: Some(memory_tracker),
        }
    }

    /// Check if lazy loading optimization is enabled
    pub fn is_lazy_loading_enabled(&self) -> bool {
        self.lazy_loading_cache.is_some()
    }

    /// Graceful shutdown of all components including lazy loading
    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Initiating graceful shutdown of AppState");

        // Shutdown lazy loading cache if enabled
        if let Some(lazy_cache) = &self.lazy_loading_cache {
            lazy_cache.shutdown().await?;
        }

        // Shutdown memory tracker monitoring if enabled
        if let Some(memory_tracker) = &self.memory_tracker {
            tracing::debug!("Stopping memory tracker monitoring");
            // Memory tracker cleanup would happen here if it had background tasks
        }

        tracing::info!("Completed graceful shutdown of AppState");
        Ok(())
    }

    /// Health check for all AppState components
    pub async fn health_check(&self) -> AppStateHealth {
        let lazy_loading_health = if let Some(lazy_cache) = &self.lazy_loading_cache {
            Some(lazy_cache.health_check().await)
        } else {
            None
        };

        let memory_health = if let Some(memory_tracker) = &self.memory_tracker {
            let stats = memory_tracker.get_memory_stats().await;
            Some(AppStateMemoryHealth {
                current_usage_mb: stats.current_memory_mb,
                health_status: stats.health_status,
            })
        } else {
            None
        };

        AppStateHealth {
            lazy_loading: lazy_loading_health,
            memory: memory_health,
            database_connected: true, // TODO: Add actual DB health check
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct AppStateHealth {
    pub lazy_loading: Option<crate::cache::lazy_loading_cache::LazyLoadingHealthStatus>,
    pub memory: Option<AppStateMemoryHealth>,
    pub database_connected: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct AppStateMemoryHealth {
    pub current_usage_mb: f64,
    pub health_status: crate::monitoring::memory_tracker::MemoryHealthStatus,
}
