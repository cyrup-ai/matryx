use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for lazy loading optimization features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LazyLoadingConfig {
    /// Cache configuration
    pub cache: CacheConfig,
    
    /// Performance settings
    pub performance: PerformanceConfig,
    
    /// Feature flags
    pub features: FeatureFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Essential members cache configuration
    pub essential_members: CacheInstanceConfig,
    
    /// Power hierarchies cache configuration
    pub power_hierarchies: CacheInstanceConfig,
    
    /// Room creators cache configuration
    pub room_creators: CacheInstanceConfig,
    
    /// Membership events cache configuration
    pub membership_events: CacheInstanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheInstanceConfig {
    /// Maximum number of entries
    pub max_capacity: u64,
    
    /// Time-to-live in seconds
    pub ttl_seconds: u64,
    
    /// Enable cache statistics
    pub enable_stats: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum acceptable database query time (ms)
    pub max_db_query_time_ms: u64,
    
    /// Maximum acceptable total processing time (ms)
    pub max_total_processing_time_ms: u64,
    
    /// Minimum required cache hit ratio
    pub min_cache_hit_ratio: f64,
    
    /// Maximum acceptable memory usage (MB)
    pub max_memory_usage_mb: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enable enhanced database optimization
    pub enable_database_optimization: bool,
    
    /// Enable multi-level caching
    pub enable_multi_level_caching: bool,
    
    /// Enable real-time cache invalidation
    pub enable_realtime_invalidation: bool,
    
    /// Enable performance monitoring
    pub enable_performance_monitoring: bool,
    
    /// Enable cache-aware filtering
    pub enable_cache_aware_filtering: bool,
}

impl Default for LazyLoadingConfig {
    fn default() -> Self {
        Self {
            cache: CacheConfig {
                essential_members: CacheInstanceConfig {
                    max_capacity: 5000,
                    ttl_seconds: 300,  // 5 minutes
                    enable_stats: true,
                },
                power_hierarchies: CacheInstanceConfig {
                    max_capacity: 10000,
                    ttl_seconds: 600,  // 10 minutes
                    enable_stats: true,
                },
                room_creators: CacheInstanceConfig {
                    max_capacity: 50000,
                    ttl_seconds: 3600,  // 1 hour
                    enable_stats: true,
                },
                membership_events: CacheInstanceConfig {
                    max_capacity: 2000,
                    ttl_seconds: 120,  // 2 minutes
                    enable_stats: true,
                },
            },
            performance: PerformanceConfig {
                max_db_query_time_ms: 50,
                max_total_processing_time_ms: 100,
                min_cache_hit_ratio: 0.80,
                max_memory_usage_mb: 100,
            },
            features: FeatureFlags {
                enable_database_optimization: true,
                enable_multi_level_caching: true,
                enable_realtime_invalidation: true,
                enable_performance_monitoring: true,
                enable_cache_aware_filtering: true,
            },
        }
    }
}

impl LazyLoadingConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();
        
        // Override with environment variables if present
        if let Ok(val) = std::env::var("LAZY_LOADING_ESSENTIAL_MEMBERS_CAPACITY") {
            if let Ok(capacity) = val.parse::<u64>() {
                config.cache.essential_members.max_capacity = capacity;
            }
        }
        
        if let Ok(val) = std::env::var("LAZY_LOADING_ESSENTIAL_MEMBERS_TTL") {
            if let Ok(ttl) = val.parse::<u64>() {
                config.cache.essential_members.ttl_seconds = ttl;
            }
        }
        
        if let Ok(val) = std::env::var("LAZY_LOADING_MAX_DB_QUERY_TIME_MS") {
            if let Ok(time_ms) = val.parse::<u64>() {
                config.performance.max_db_query_time_ms = time_ms;
            }
        }
        
        if let Ok(val) = std::env::var("LAZY_LOADING_ENABLE_DATABASE_OPTIMIZATION") {
            if let Ok(enabled) = val.parse::<bool>() {
                config.features.enable_database_optimization = enabled;
            }
        }
        
        config
    }
    
    /// Convert cache TTL seconds to Duration
    pub fn essential_members_ttl(&self) -> Duration {
        Duration::from_secs(self.cache.essential_members.ttl_seconds)
    }
    
    pub fn power_hierarchies_ttl(&self) -> Duration {
        Duration::from_secs(self.cache.power_hierarchies.ttl_seconds)
    }
    
    pub fn room_creators_ttl(&self) -> Duration {
        Duration::from_secs(self.cache.room_creators.ttl_seconds)
    }
    
    pub fn membership_events_ttl(&self) -> Duration {
        Duration::from_secs(self.cache.membership_events.ttl_seconds)
    }
}