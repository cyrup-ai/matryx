//! Performance monitoring for filtering operations
//!
//! This module provides metrics collection for Matrix sync filtering
//! to monitor performance and usage patterns using Prometheus metrics.

use lazy_static::lazy_static;
use prometheus::{Histogram, IntCounter, register_histogram, register_int_counter};
use std::time::Instant;

lazy_static! {
    static ref FILTER_OPERATIONS: IntCounter = register_int_counter!(
        "matrix_filter_operations_total",
        "Total number of filter operations"
    )
    .unwrap();
    static ref FILTER_PROCESSING_TIME: Histogram =
        register_histogram!("matrix_filter_processing_seconds", "Time spent processing filters")
            .unwrap();
    static ref EVENTS_FILTERED: IntCounter =
        register_int_counter!("matrix_events_filtered_total", "Total number of events filtered")
            .unwrap();
    static ref CACHE_HITS: IntCounter = register_int_counter!(
        "matrix_filter_cache_hits_total",
        "Total number of filter cache hits"
    )
    .unwrap();
    static ref CACHE_MISSES: IntCounter = register_int_counter!(
        "matrix_filter_cache_misses_total",
        "Total number of filter cache misses"
    )
    .unwrap();
    static ref LIVE_QUERY_OPERATIONS: IntCounter = register_int_counter!(
        "matrix_live_query_operations_total",
        "Total number of live query operations"
    )
    .unwrap();
}

/// Performance monitoring for filtering operations
pub struct FilterMetrics;

impl FilterMetrics {
    /// Record a filter operation with timing and event count
    pub fn record_filter_operation(
        filter_type: &str,
        processing_time: f64,
        events_processed: usize,
    ) {
        FILTER_OPERATIONS.inc();
        FILTER_PROCESSING_TIME.observe(processing_time);
        EVENTS_FILTERED.inc_by(events_processed as u64);

        // Also log for debugging
        tracing::debug!(
            filter_type = filter_type,
            processing_time_ms = processing_time * 1000.0,
            events_processed = events_processed,
            "Filter operation completed"
        );

        // Log slow operations
        if processing_time > 0.01 {
            // Log slow operations (>10ms)
            tracing::warn!(
                filter_type = filter_type,
                processing_time_ms = processing_time * 1000.0,
                events_processed = events_processed,
                "Slow filter operation detected"
            );
        }
    }

    /// Record filter cache hit/miss
    pub fn record_cache_operation(operation: &str, hit: bool) {
        if hit {
            CACHE_HITS.inc();
        } else {
            CACHE_MISSES.inc();
        }

        tracing::debug!(operation = operation, cache_hit = hit, "Filter cache operation");
    }

    /// Record live query operation
    pub fn record_live_query_operation(operation: &str, user_count: usize) {
        LIVE_QUERY_OPERATIONS.inc();

        tracing::info!(operation = operation, active_users = user_count, "Live query operation");
    }
}

/// Timer helper for measuring filter operation performance
pub struct FilterTimer {
    start: Instant,
    filter_type: String,
}

impl FilterTimer {
    pub fn new(filter_type: &str) -> Self {
        Self {
            start: Instant::now(),
            filter_type: filter_type.to_string(),
        }
    }

    pub fn finish(self, events_processed: usize) {
        let duration = self.start.elapsed().as_secs_f64();
        FilterMetrics::record_filter_operation(&self.filter_type, duration, events_processed);
    }
}
