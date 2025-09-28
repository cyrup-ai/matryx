use crate::auth::{MatrixSessionService, oauth2::OAuth2Service, uia::UiaService};
use crate::config::ServerConfig;
use crate::federation::dns_resolver::MatrixDnsResolver;
use crate::federation::media_client::FederationMediaClient;
use crate::cache::lazy_loading_cache::LazyLoadingCache;
use crate::federation::event_signer::EventSigner;
use crate::metrics::lazy_loading_benchmarks::{LazyLoadingBenchmarks, LazyLoadingBenchmarkConfig};
use crate::metrics::lazy_loading_metrics::LazyLoadingMetrics;
use crate::monitoring::lazy_loading_alerts::{LazyLoadingAlerts, AlertingConfig, ConsoleNotificationSender};
use crate::monitoring::memory_tracker::LazyLoadingMemoryTracker;
use matryx_surrealdb::repository::push::PushRepository;
use matryx_surrealdb::repository::{
    MentionRepository,
    ServerNoticeRepository,
    ThreadRepository,
    database_health::DatabaseHealthRepository,
    event::EventRepository,
    membership::MembershipRepository,
    metrics::HealthStatus,
    monitoring::MonitoringRepository,
    oauth2::OAuth2Repository,
    performance::PerformanceRepository,
    relations::RelationsRepository,
    room::RoomRepository,
    room_operations::RoomOperationsService,
    threads::ThreadsRepository,
    uia::UiaRepository,
};
use std::sync::Arc;
use surrealdb::{Surreal, engine::any::Any};

#[derive(Clone)]
pub struct AppState {
    pub db: Surreal<Any>,
    pub session_service: Arc<MatrixSessionService<Any>>,
    pub oauth2_service: Arc<OAuth2Service<Any>>,
    pub uia_service: Arc<UiaService>,
    pub homeserver_name: String,
    pub config: &'static ServerConfig,
    pub http_client: Arc<reqwest::Client>,
    pub event_signer: Arc<EventSigner>,
    pub dns_resolver: Arc<MatrixDnsResolver>,
    pub federation_media_client: Arc<FederationMediaClient>,
    pub push_engine: Arc<PushRepository<Any>>,
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
    /// Performance alerting system for lazy loading degradation detection
    pub lazy_loading_alerts: Option<Arc<LazyLoadingAlerts>>,
    /// Performance benchmarking system for lazy loading optimization
    pub lazy_loading_benchmarks: Option<Arc<LazyLoadingBenchmarks>>,
    /// Database health monitoring repository
    pub database_health_repo: Arc<DatabaseHealthRepository<Any>>,
}

impl AppState {
    pub fn new(
        db: Surreal<Any>,
        session_service: Arc<MatrixSessionService<Any>>,
        homeserver_name: String,
        config: &'static ServerConfig,
        http_client: Arc<reqwest::Client>,
        event_signer: Arc<EventSigner>,
        dns_resolver: Arc<MatrixDnsResolver>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Initialize OAuth2 service
        let oauth2_repo = OAuth2Repository::new(db.clone());
        let oauth2_service = Arc::new(OAuth2Service::new(
            oauth2_repo,
            session_service.clone(),
            homeserver_name.clone(),
        ));
        
        // Initialize UIA service
        let uia_repo = UiaRepository::new(db.clone());
        let uia_service = Arc::new(UiaService::new(uia_repo));
        
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

        // Initialize federation media client
        let federation_media_client = Arc::new(FederationMediaClient::new(
            http_client.clone(),
            event_signer.clone(),
            homeserver_name.clone(),
        ));

        // Initialize push engine
        let push_engine = Arc::new(PushRepository::new(db.clone()));

        // Initialize database health repository
        let database_health_repo = Arc::new(DatabaseHealthRepository::new(db.clone()));

        Ok(Self {
            db,
            session_service,
            oauth2_service,
            uia_service,
            homeserver_name,
            config,
            http_client,
            event_signer,
            dns_resolver,
            federation_media_client,
            push_engine,
            thread_repository,
            mention_repository,
            server_notice_repository,
            room_operations,
            lazy_loading_cache: None,
            lazy_loading_metrics: None,
            memory_tracker: None,
            lazy_loading_alerts: None,
            lazy_loading_benchmarks: None,
            database_health_repo,
        })
    }

    /// Create AppState with enhanced lazy loading optimization enabled
    pub fn with_lazy_loading_optimization(
        db: Surreal<Any>,
        session_service: Arc<MatrixSessionService<Any>>,
        homeserver_name: String,
        config: &'static ServerConfig,
        http_client: Arc<reqwest::Client>,
        event_signer: Arc<EventSigner>,
        dns_resolver: Arc<MatrixDnsResolver>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Initialize OAuth2 service
        let oauth2_repo = OAuth2Repository::new(db.clone());
        let oauth2_service = Arc::new(OAuth2Service::new(
            oauth2_repo,
            session_service.clone(),
            homeserver_name.clone(),
        ));
        
        // Initialize UIA service
        let uia_repo = UiaRepository::new(db.clone());
        let uia_service = Arc::new(UiaService::new(uia_repo));
        
        // Initialize lazy loading components
        let lazy_cache = Arc::new(LazyLoadingCache::new());

        // Create repositories for metrics and monitoring
        let performance_repo = Arc::new(PerformanceRepository::new(db.clone()));
        let monitoring_repo = Arc::new(MonitoringRepository::new(db.clone()));

        let metrics = Arc::new(LazyLoadingMetrics::new(performance_repo.clone()));
        let memory_tracker =
            Arc::new(LazyLoadingMemoryTracker::new(performance_repo, monitoring_repo));

        // Initialize alerting system with production-quality configuration
        let alert_config = AlertingConfig::default();
        let notification_sender = Arc::new(ConsoleNotificationSender);
        let lazy_loading_alerts = Arc::new(LazyLoadingAlerts::new(
            alert_config,
            notification_sender,
            metrics.clone(),
        ));

        // Initialize benchmarking system with production-quality configuration
        let benchmark_config = LazyLoadingBenchmarkConfig::default();
        let lazy_loading_benchmarks = Arc::new(LazyLoadingBenchmarks::new(benchmark_config));

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

        // Initialize federation media client
        let federation_media_client = Arc::new(FederationMediaClient::new(
            http_client.clone(),
            event_signer.clone(),
            homeserver_name.clone(),
        ));

        // Initialize push engine
        let push_engine = Arc::new(PushRepository::new(db.clone()));

        // Initialize database health repository
        let database_health_repo = Arc::new(DatabaseHealthRepository::new(db.clone()));

        Ok(Self {
            db,
            session_service,
            oauth2_service,
            uia_service,
            homeserver_name,
            config,
            http_client,
            event_signer,
            dns_resolver,
            federation_media_client,
            push_engine,
            thread_repository,
            mention_repository,
            server_notice_repository,
            room_operations,
            lazy_loading_cache: Some(lazy_cache),
            lazy_loading_metrics: Some(metrics),
            memory_tracker: Some(memory_tracker),
            lazy_loading_alerts: Some(lazy_loading_alerts),
            lazy_loading_benchmarks: Some(lazy_loading_benchmarks),
            database_health_repo,
        })
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
            
            // Log final memory statistics before shutdown using available method
            let memory_stats = memory_tracker.get_memory_stats().await;
            tracing::info!(
                "Memory tracker shutdown - Current: {:.2} MB, Peak: {:.2} MB, Baseline: {:.2} MB",
                memory_stats.current_memory_mb,
                memory_stats.peak_memory_mb,
                memory_stats.baseline_memory_mb
            );
            tracing::info!("Memory tracker shutdown completed");
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

        // Check actual database health
        let database_connected = match self.database_health_repo.check_connectivity().await {
            Ok(health_status) => health_status.status == HealthStatus::Healthy,
            Err(_) => false,
        };

        AppStateHealth {
            lazy_loading: lazy_loading_health,
            memory: memory_health,
            database_connected,
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

impl AppStateMemoryHealth {
    pub fn is_healthy(&self) -> bool {
        matches!(self.health_status, crate::monitoring::memory_tracker::MemoryHealthStatus::Healthy)
    }
}
