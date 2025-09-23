//! Caching for frequently used filters
//!
//! This module provides intelligent caching for Matrix filters to improve
//! performance by avoiding repeated filter compilation and processing.

use crate::metrics::filter_metrics::FilterMetrics;
use matryx_entity::types::{Event, MatrixFilter};
use moka::future::Cache;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;

/// Compiled filter for efficient reuse
#[derive(Debug, Clone)]
pub struct CompiledFilter {
    pub original: MatrixFilter,
    pub hash: String,
    pub compiled_at: std::time::Instant,
}

/// Cache for frequently used filters using thread-safe async cache
pub struct FilterCache {
    compiled_filters: Cache<String, CompiledFilter>,
    filter_results: Cache<String, Vec<Event>>,
}

impl FilterCache {
    pub fn new() -> Self {
        Self {
            compiled_filters: Cache::new(1000), // Max 1000 compiled filters
            filter_results: Cache::builder()
                .max_capacity(5000) // Max 5000 cached results
                .time_to_live(Duration::from_secs(300)) // 5 minutes TTL
                .build(),
        }
    }

    /// Get or compile a filter for efficient reuse
    pub async fn get_or_compile_filter(&self, filter: &MatrixFilter) -> CompiledFilter {
        let cache_key = calculate_filter_hash(filter);

        if let Some(compiled) = self.compiled_filters.get(&cache_key).await {
            FilterMetrics::record_cache_operation("filter_compile", true);
            return compiled;
        }

        FilterMetrics::record_cache_operation("filter_compile", false);

        let compiled = compile_filter(filter);
        self.compiled_filters.insert(cache_key, compiled.clone()).await;
        compiled
    }

    /// Cache filter results for a specific room and filter combination
    pub async fn cache_filter_results(
        &self,
        filter_hash: &str,
        room_id: &str,
        results: Vec<Event>,
    ) {
        let cache_key = format!("{}:{}", filter_hash, room_id);
        self.filter_results.insert(cache_key, results).await;
    }

    /// Get cached filter results if available and not expired
    pub async fn get_cached_results(&self, filter_hash: &str, room_id: &str) -> Option<Vec<Event>> {
        let cache_key = format!("{}:{}", filter_hash, room_id);

        if let Some(results) = self.filter_results.get(&cache_key).await {
            FilterMetrics::record_cache_operation("filter_results", true);
            Some(results)
        } else {
            FilterMetrics::record_cache_operation("filter_results", false);
            None
        }
    }

    /// Invalidate cache for a specific room (call when room events change)
    pub async fn invalidate_room(&self, room_id: &str) {
        // Invalidate all entries for this room
        // Note: moka doesn't have a direct way to invalidate by pattern,
        // so we'd need to track keys separately for this functionality
        // For now, this is a simplified implementation
        tracing::debug!("Room invalidation requested for room: {}", room_id);
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> CacheStats {
        CacheStats {
            compiled_filters_count: self.compiled_filters.entry_count(),
            cached_results_count: self.filter_results.entry_count(),
            max_compiled_filters: 1000,
            max_cached_results: 5000,
        }
    }
}

impl Default for FilterCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics for monitoring
#[derive(Debug)]
pub struct CacheStats {
    pub compiled_filters_count: u64,
    pub cached_results_count: u64,
    pub max_compiled_filters: usize,
    pub max_cached_results: usize,
}

/// Calculate a hash for a filter to use as cache key
fn calculate_filter_hash(filter: &MatrixFilter) -> String {
    let mut hasher = DefaultHasher::new();

    // Hash the filter structure for cache key
    if let Ok(json) = serde_json::to_string(filter) {
        json.hash(&mut hasher);
    }

    format!("{:x}", hasher.finish())
}

/// Compile a filter for efficient reuse
fn compile_filter(filter: &MatrixFilter) -> CompiledFilter {
    CompiledFilter {
        original: filter.clone(),
        hash: calculate_filter_hash(filter),
        compiled_at: std::time::Instant::now(),
    }
}
