use crate::{
    push::{
        rules::{PushRuleEngine, PushAction, RoomContext},
        gateway::{PushGateway, PushNotification, NotificationData, NotificationCounts, DeviceInfo, PushError},
    },
    config::server_config::PushCacheConfig,
};
use matryx_entity::PDU;
use surrealdb::{Surreal, engine::local::Db};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Duration;
use moka::future::Cache;
use tokio::sync::Mutex;
use tracing::{error, info, warn, debug};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pusher {
    pub pusher_id: String,
    pub user_id: String,
    pub kind: String, // "http" | "email" etc.
    pub app_id: String,
    pub app_display_name: String,
    pub device_display_name: String,
    pub profile_tag: Option<String>,
    pub lang: String,
    pub data: PusherData,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PusherData {
    pub url: Option<String>,
    pub format: Option<String>, // "event_id_only" or full notification
}

#[derive(Debug, Clone)]
pub struct RoomMember {
    pub user_id: String,
    pub display_name: Option<String>,
    pub power_level: i64,
}

// PushCacheConfig is now imported from config::server_config

#[derive(Debug, Default, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
    pub created_at: std::time::Instant,
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
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }
}

pub struct PushEngine {
    rule_engine: PushRuleEngine,
    gateways: Cache<String, Arc<PushGateway>>,
    http_client: reqwest::Client,
    db: Arc<Surreal<Db>>,
    cache_config: PushCacheConfig,
    cache_stats: Arc<Mutex<CacheStats>>,
}

impl PushEngine {
    pub fn new(db: Arc<Surreal<Db>>) -> Result<Self, PushError> {
        Self::with_config(db, PushCacheConfig::default())
    }
    
    pub fn with_config(db: Arc<Surreal<Db>>, cache_config: PushCacheConfig) -> Result<Self, PushError> {
        let default_rules = PushRuleEngine::get_default_rules();
        let rule_engine = PushRuleEngine::new(default_rules);
        
        // Configure high-performance cache with TTL and capacity limits
        let gateways = Cache::builder()
            .time_to_live(Duration::from_secs(cache_config.ttl_seconds))
            .max_capacity(cache_config.max_capacity)
            .build();
        
        // Configure HTTP client with connection pooling
        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(10)                    // 10 idle connections per host
            .pool_idle_timeout(Duration::from_secs(30))    // 30s idle timeout
            .timeout(Duration::from_secs(30))              // 30s request timeout
            .tcp_keepalive(Duration::from_secs(60))        // TCP keep-alive
            .build()
            .map_err(PushError::HttpError)?;
        
        Ok(Self {
            rule_engine,
            gateways,
            http_client,
            db,
            cache_config,
            cache_stats: Arc::new(Mutex::new(CacheStats::new())),
        })
    }

    /// Get or create gateway with caching optimization
    async fn get_or_create_gateway(&self, gateway_url: &str) -> Result<Arc<PushGateway>, PushError> {
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
            self.http_client.clone(),  // Reuse connection pool
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

        // 1. Get room members
        let members = self.get_room_members(room_id).await?;
        let member_count = members.len() as u64;
        
        // 2. Get room power levels
        let power_levels = self.get_room_power_levels(room_id).await?;

        // 3. For each member, evaluate push rules
        for member in members {
            if member.user_id == event.sender {
                continue; // Don't notify sender
            }

            let room_context = RoomContext {
                room_id: room_id.to_string(),
                member_count,
                user_display_name: member.display_name.clone(),
                power_levels: power_levels.clone(),
            };

            let actions = self.rule_engine.evaluate_event(event, &room_context);
            
            if actions.contains(&PushAction::Notify) {
                // 4. Get user's pushers
                let pushers = self.get_user_pushers(&member.user_id).await?;
                
                // 5. Send notifications
                for pusher in pushers {
                    if let Err(e) = self.send_push_notification(&pusher, event, &actions, &room_context).await {
                        error!("Failed to send push notification to {}: {}", pusher.pusher_id, e);
                    }
                }
            } else {
                debug!("Push rules determined not to notify user {} for event {}", member.user_id, event.event_id);
            }
        }

        Ok(())
    }

    async fn get_room_members(&self, room_id: &str) -> Result<Vec<RoomMember>, PushError> {
        let query = "
            SELECT user_id, content.displayname as display_name, content.membership
            FROM room_memberships 
            WHERE room_id = $room_id AND content.membership = 'join'
        ";
        
        let mut result = self.db
            .query(query)
            .bind(("room_id", room_id))
            .await
            .map_err(|e| PushError::HttpError(reqwest::Error::from(e)))?;

        let members: Vec<serde_json::Value> = result
            .take(0)
            .map_err(|e| PushError::HttpError(reqwest::Error::from(e)))?;

        let room_members = members
            .into_iter()
            .filter_map(|member| {
                let user_id = member.get("user_id")?.as_str()?.to_string();
                let display_name = member.get("display_name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                
                Some(RoomMember {
                    user_id,
                    display_name,
                    power_level: 0, // Default power level
                })
            })
            .collect();

        Ok(room_members)
    }

    async fn get_room_power_levels(&self, room_id: &str) -> Result<HashMap<String, i64>, PushError> {
        let query = "
            SELECT content.users
            FROM room_state_events 
            WHERE room_id = $room_id AND type = 'm.room.power_levels' AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        
        let mut result = self.db
            .query(query)
            .bind(("room_id", room_id))
            .await
            .map_err(|e| PushError::HttpError(reqwest::Error::from(e)))?;

        let power_level_events: Vec<serde_json::Value> = result
            .take(0)
            .map_err(|e| PushError::HttpError(reqwest::Error::from(e)))?;

        let mut power_levels = HashMap::new();
        
        if let Some(event) = power_level_events.first() {
            if let Some(users) = event.get("users").and_then(|u| u.as_object()) {
                for (user_id, level) in users {
                    if let Some(level_num) = level.as_i64() {
                        power_levels.insert(user_id.clone(), level_num);
                    }
                }
            }
        }

        Ok(power_levels)
    }

    async fn get_user_pushers(&self, user_id: &str) -> Result<Vec<Pusher>, PushError> {
        let query = "
            SELECT * FROM pushers 
            WHERE user_id = $user_id AND kind = 'http'
        ";
        
        let mut result = self.db
            .query(query)
            .bind(("user_id", user_id))
            .await
            .map_err(|e| PushError::HttpError(reqwest::Error::from(e)))?;

        let pusher_records: Vec<serde_json::Value> = result
            .take(0)
            .map_err(|e| PushError::HttpError(reqwest::Error::from(e)))?;

        let pushers = pusher_records
            .into_iter()
            .filter_map(|record| {
                serde_json::from_value(record).ok()
            })
            .collect();

        Ok(pushers)
    }

    async fn send_push_notification(
        &self,
        pusher: &Pusher,
        event: &PDU,
        actions: &[PushAction],
        room_context: &RoomContext,
    ) -> Result<(), PushError> {
        // Get or create gateway for this pusher
        let gateway_url = pusher.data.url.as_ref()
            .ok_or_else(|| PushError::InvalidUrl("Pusher has no gateway URL".to_string()))?;

        // Get cached or create new gateway
        let gateway = self.get_or_create_gateway(gateway_url).await?;

        // Send notification with error handling and cache invalidation
        match self.send_with_gateway(&gateway, pusher, event, actions, room_context).await {
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
        let device_info = DeviceInfo {
            app_id: pusher.app_id.clone(),
            pushkey: pusher.pusher_id.clone(),
            pushkey_ts: Some(pusher.created_at),
            data: Some(serde_json::to_value(&pusher.data)?),
            tweaks: if tweaks.is_empty() { None } else { Some(serde_json::Value::Object(tweaks)) },
        };

        // Determine notification content based on format
        let content = if pusher.data.format.as_deref() == Some("event_id_only") {
            None // Don't include content for event_id_only format
        } else {
            Some(event.content.clone())
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
            }
        }
    }

    async fn get_notification_counts(&self, user_id: &str) -> Result<NotificationCounts, PushError> {
        // This would query the database for unread counts
        // For now, return default counts
        Ok(NotificationCounts {
            unread: Some(1),
            missed_calls: None,
        })
    }

    async fn get_room_name(&self, room_id: &str) -> Result<String, PushError> {
        let query = "
            SELECT content.name
            FROM room_state_events 
            WHERE room_id = $room_id AND type = 'm.room.name' AND state_key = ''
            ORDER BY origin_server_ts DESC
            LIMIT 1
        ";
        
        let mut result = self.db
            .query(query)
            .bind(("room_id", room_id))
            .await
            .map_err(|e| PushError::HttpError(reqwest::Error::from(e)))?;

        let name_events: Vec<serde_json::Value> = result
            .take(0)
            .map_err(|e| PushError::HttpError(reqwest::Error::from(e)))?;

        if let Some(event) = name_events.first() {
            if let Some(name) = event.get("name").and_then(|n| n.as_str()) {
                return Ok(name.to_string());
            }
        }

        Ok(format!("Room {}", room_id))
    }

    async fn get_user_display_name(&self, user_id: &str) -> Result<Option<String>, PushError> {
        let query = "
            SELECT content.displayname
            FROM user_profiles 
            WHERE user_id = $user_id
            LIMIT 1
        ";
        
        let mut result = self.db
            .query(query)
            .bind(("user_id", user_id))
            .await
            .map_err(|e| PushError::HttpError(reqwest::Error::from(e)))?;

        let profile_records: Vec<serde_json::Value> = result
            .take(0)
            .map_err(|e| PushError::HttpError(reqwest::Error::from(e)))?;

        if let Some(profile) = profile_records.first() {
            if let Some(display_name) = profile.get("displayname").and_then(|n| n.as_str()) {
                return Ok(Some(display_name.to_string()));
            }
        }

        Ok(None)
    }

    fn is_user_target(&self, event: &PDU, user_id: &str) -> bool {
        // Check if this is a membership event targeting the user
        event.event_type == "m.room.member" && event.state_key.as_deref() == Some(user_id)
    }
}