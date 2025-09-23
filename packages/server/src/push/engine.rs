use crate::{
    config::server_config::PushCacheConfig,
    push::{
        gateway::{
            NotificationCounts,
            NotificationData,
            PushDeviceInfo,
            PushError,
            PushGateway,
            PushNotification,
        },
        rules::PushRuleEngine,
    },
};
use matryx_entity::{PDU, Pusher, PusherData};
use matryx_surrealdb::repository::PusherRepository;
use matryx_surrealdb::repository::push_service::{
    Event as PushEvent,
    PushAction,
    PushCleanupResult,
    PushReceipt,
    PushService,
    RoomContext,
};
use matryx_surrealdb::repository::pusher::RoomMember as PusherRoomMember;
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use surrealdb::engine::any::Any;
use surrealdb::{Surreal, engine::local::Db};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

// PushCacheConfig is now imported from config::server_config

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
    pub created_at: std::time::Instant,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            invalidations: 0,
            created_at: std::time::Instant::now(),
        }
    }
}

impl CacheStats {
    pub fn new() -> Self {
        Self {
            hits: 0,
            misses: 0,
            invalidations: 0,
            created_at: std::time::Instant::now(),
        }
    }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

pub struct PushEngine {
    rule_engine: PushRuleEngine,
    gateways: Cache<String, Arc<PushGateway>>,
    http_client: reqwest::Client,
    db: Arc<Surreal<Any>>,
    pusher_repository: PusherRepository<Any>,
    push_service: PushService,
    cache_config: PushCacheConfig,
    cache_stats: Arc<Mutex<CacheStats>>,
}

impl PushEngine {
    pub fn new(db: Arc<Surreal<Any>>) -> Result<Self, PushError> {
        Self::with_config(db, PushCacheConfig::default())
    }

    pub fn with_config(
        db: Arc<Surreal<Any>>,
        cache_config: PushCacheConfig,
    ) -> Result<Self, PushError> {
        let rule_engine = PushRuleEngine::new(db.as_ref().clone().into());

        // Configure high-performance cache with TTL and capacity limits
        let gateways = Cache::builder()
            .time_to_live(Duration::from_secs(cache_config.ttl_seconds))
            .max_capacity(cache_config.max_capacity)
            .build();

        // Configure HTTP client with connection pooling
        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(10) // 10 idle connections per host
            .pool_idle_timeout(Duration::from_secs(30)) // 30s idle timeout
            .timeout(Duration::from_secs(30)) // 30s request timeout
            .tcp_keepalive(Duration::from_secs(60)) // TCP keep-alive
            .build()
            .map_err(PushError::HttpError)?;

        let pusher_repository = PusherRepository::new(db.as_ref().clone());
        let push_service = PushService::new(db.as_ref().clone().into());

        Ok(Self {
            rule_engine,
            gateways,
            http_client,
            db,
            pusher_repository,
            push_service,
            cache_config,
            cache_stats: Arc::new(Mutex::new(CacheStats::new())),
        })
    }

    /// Get or create gateway with caching optimization
    async fn get_or_create_gateway(
        &self,
        gateway_url: &str,
    ) -> Result<Arc<PushGateway>, PushError> {
        // Check cache first (fast path)
        if let Some(gateway) = self.gateways.get(gateway_url).await {
            self.record_cache_hit().await;
            debug!("Cache hit for gateway: {}", gateway_url);
            return Ok(gateway);
        }

        self.record_cache_miss().await;
        debug!("Cache miss for gateway: {}, creating new instance", gateway_url);

        // Create new gateway with shared HTTP client (connection pooling)
        let gateway = Arc::new(PushGateway::with_client(
            gateway_url.to_string(),
            self.http_client.clone(), // Reuse connection pool
        )?);

        // Cache the gateway for future requests
        self.gateways.insert(gateway_url.to_string(), gateway.clone()).await;

        info!("Created and cached new gateway: {}", gateway_url);
        Ok(gateway)
    }

    /// Invalidate failed gateway from cache
    async fn invalidate_gateway(&self, gateway_url: &str, reason: &str) {
        self.gateways.invalidate(gateway_url).await;
        warn!("Invalidated gateway {} from cache: {}", gateway_url, reason);
        self.record_cache_invalidation().await;
    }

    async fn record_cache_hit(&self) {
        if let Ok(mut stats) = self.cache_stats.try_lock() {
            stats.hits += 1;
        }
    }

    async fn record_cache_miss(&self) {
        if let Ok(mut stats) = self.cache_stats.try_lock() {
            stats.misses += 1;
        }
    }

    async fn record_cache_invalidation(&self) {
        if let Ok(mut stats) = self.cache_stats.try_lock() {
            stats.invalidations += 1;
        }
    }

    pub async fn get_cache_stats(&self) -> CacheStats {
        self.cache_stats.lock().await.clone()
    }

    pub async fn log_cache_performance(&self) {
        let stats = self.get_cache_stats().await;
        let cache_size = self.gateways.entry_count();

        info!(
            "Push gateway cache stats: hit_ratio={:.2}%, size={}, hits={}, misses={}, invalidations={}",
            stats.hit_ratio() * 100.0,
            cache_size,
            stats.hits,
            stats.misses,
            stats.invalidations
        );
    }

    pub async fn process_event(&self, event: &PDU, room_id: &str) -> Result<(), PushError> {
        info!("Processing push notifications for event {} in room {}", event.event_id, room_id);

        // Convert PDU to PushEvent
        let push_event = PushEvent {
            event_id: event.event_id.clone(),
            event_type: event.event_type.clone(),
            sender: event.sender.clone(),
            content: serde_json::to_value(&event.content)?,
            state_key: event.state_key.clone(),
        };

        // Use PushService to process the event
        match self.push_service.process_event_for_push(&push_event, room_id).await {
            Ok(notifications) => {
                info!(
                    "Generated {} push notifications for event {}",
                    notifications.len(),
                    event.event_id
                );

                // Send each notification
                for notification in notifications {
                    if let Err(e) = self.push_service.send_push_notification(&notification).await {
                        error!(
                            "Failed to send push notification {}: {}",
                            notification.notification_id, e
                        );
                    } else {
                        info!(
                            "Successfully sent push notification {}",
                            notification.notification_id
                        );
                    }
                }
            },
            Err(e) => {
                error!("Failed to process event for push notifications: {}", e);
                return Err(PushError::RepositoryError(e));
            },
        }

        Ok(())
    }

    async fn get_room_members(&self, room_id: &str) -> Result<Vec<PusherRoomMember>, PushError> {
        Ok(self.pusher_repository.get_room_members_for_push(room_id).await?)
    }

    async fn get_room_power_levels(
        &self,
        room_id: &str,
    ) -> Result<HashMap<String, i64>, PushError> {
        Ok(self.pusher_repository.get_room_power_levels(room_id).await?)
    }

    async fn get_user_pushers(&self, user_id: &str) -> Result<Vec<Pusher>, PushError> {
        Ok(self.pusher_repository.get_user_pushers(user_id).await?)
    }

    async fn send_push_notification(
        &self,
        pusher: &Pusher,
        event: &PDU,
        actions: &[PushAction],
        room_context: &RoomContext,
    ) -> Result<(), PushError> {
        // Get or create gateway for this pusher
        let gateway_url = pusher
            .data
            .url
            .as_ref()
            .ok_or_else(|| PushError::InvalidUrl("Pusher has no gateway URL".to_string()))?;

        // Get cached or create new gateway
        let gateway = self.get_or_create_gateway(gateway_url).await?;

        // Send notification with error handling and cache invalidation
        match self
            .send_with_gateway(&gateway, pusher, event, actions, room_context)
            .await
        {
            Ok(()) => Ok(()),
            Err(PushError::GatewayError(status)) if status.is_client_error() => {
                // 4xx errors indicate gateway configuration issues - invalidate cache
                self.invalidate_gateway(gateway_url, &format!("HTTP {}", status)).await;
                Err(PushError::GatewayError(status))
            },
            Err(e) => Err(e), // Other errors don't invalidate cache
        }
    }

    async fn send_with_gateway(
        &self,
        gateway: &PushGateway,
        pusher: &Pusher,
        event: &PDU,
        actions: &[PushAction],
        room_context: &RoomContext,
    ) -> Result<(), PushError> {
        // Get notification counts for user
        let counts = self.get_notification_counts(&pusher.user_id).await?;

        // Extract tweaks from actions
        let mut tweaks = serde_json::Map::new();
        for action in actions {
            if let PushAction::SetTweak { set_tweak, value } = action {
                tweaks.insert(set_tweak.clone(), value.clone());
            }
        }

        // Build device info
        let device_info = crate::push::gateway::PushDeviceInfo {
            app_id: pusher.app_id.clone(),
            pushkey: pusher.pusher_id.clone(),
            pushkey_ts: Some(pusher.created_at),
            data: Some(serde_json::to_value(&pusher.data)?),
            tweaks: if tweaks.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(tweaks))
            },
        };

        // Determine notification content based on format
        let content = if pusher.data.format.as_deref() == Some("event_id_only") {
            None // Don't include content for event_id_only format
        } else {
            Some(serde_json::to_value(&event.content)?)
        };

        let notification = PushNotification {
            notification: NotificationData {
                content,
                counts,
                devices: vec![device_info],
                event_id: Some(event.event_id.clone()),
                prio: "high".to_string(), // Could be determined by actions
                room_id: Some(room_context.room_id.clone()),
                room_name: self.get_room_name(&room_context.room_id).await.ok(),
                sender: Some(event.sender.clone()),
                sender_display_name: self.get_user_display_name(&event.sender).await.ok().flatten(),
                type_: Some(event.event_type.clone()),
                user_is_target: Some(self.is_user_target(event, &pusher.user_id)),
            },
        };

        // Send with retry
        match gateway.send_notification_with_retry(notification, 3).await {
            Ok(response) => {
                if !response.rejected.is_empty() {
                    warn!("Some pushkeys were rejected: {:?}", response.rejected);
                    // In production, we'd remove rejected pushkeys from database
                }
                info!("Push notification sent successfully to {}", pusher.pusher_id);
                Ok(())
            },
            Err(e) => {
                error!("Failed to send push notification: {}", e);
                Err(e)
            },
        }
    }

    async fn get_notification_counts(
        &self,
        user_id: &str,
    ) -> Result<NotificationCounts, PushError> {
        // This would query the database for unread counts
        // For now, return default counts
        Ok(NotificationCounts { unread: Some(1), missed_calls: None })
    }

    async fn get_room_name(&self, room_id: &str) -> Result<String, PushError> {
        Ok(self.pusher_repository.get_room_name(room_id).await?)
    }

    async fn get_user_display_name(&self, user_id: &str) -> Result<Option<String>, PushError> {
        Ok(self.pusher_repository.get_user_display_name(user_id).await?)
    }

    fn is_user_target(&self, event: &PDU, user_id: &str) -> bool {
        // Check if this is a membership event targeting the user
        event.event_type == "m.room.member" && event.state_key.as_deref() == Some(user_id)
    }

    /// Register a pusher for a user using the PushService
    pub async fn register_pusher(
        &self,
        user_id: &str,
        pusher: &matryx_surrealdb::repository::push_gateway::Pusher,
    ) -> Result<(), PushError> {
        self.push_service
            .register_pusher(user_id, pusher)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Remove a pusher for a user using the PushService
    pub async fn remove_pusher(&self, user_id: &str, pusher_key: &str) -> Result<(), PushError> {
        self.push_service
            .remove_pusher(user_id, pusher_key)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Get user's pushers using the PushService
    pub async fn get_user_pushers_via_service(
        &self,
        user_id: &str,
    ) -> Result<Vec<matryx_surrealdb::repository::push_gateway::Pusher>, PushError> {
        self.push_service
            .get_user_pushers(user_id)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Get pending notifications for processing
    pub async fn get_pending_notifications(
        &self,
        limit: Option<u32>,
    ) -> Result<Vec<matryx_surrealdb::repository::push_notification::PushNotification>, PushError>
    {
        self.push_service
            .get_pending_notifications(limit)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Process pending notifications in batches
    pub async fn process_pending_notifications(&self, batch_size: u32) -> Result<u64, PushError> {
        self.push_service
            .process_pending_notifications(batch_size)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Cleanup old push data
    pub async fn cleanup_push_data(&self) -> Result<PushCleanupResult, PushError> {
        self.push_service
            .cleanup_push_data()
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Get push statistics for a pusher
    pub async fn get_push_statistics(
        &self,
        pusher_key: &str,
    ) -> Result<matryx_surrealdb::repository::push_gateway::PushStatistics, PushError> {
        self.push_service
            .get_push_statistics(pusher_key)
            .await
            .map_err(PushError::RepositoryError)
    }

    /// Handle push receipt
    pub async fn handle_push_receipt(
        &self,
        notification_id: &str,
        receipt: &PushReceipt,
    ) -> Result<(), PushError> {
        self.push_service
            .handle_push_receipt(notification_id, receipt)
            .await
            .map_err(PushError::RepositoryError)
    }
}
