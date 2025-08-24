use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// These metric constants have been removed since they're unused

// Global metric state
static METRICS_ENABLED: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(0));
static QUERY_DURATIONS: Lazy<Arc<Mutex<Vec<Duration>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));
static MUTATION_DURATIONS: Lazy<Arc<Mutex<Vec<Duration>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));
static MAX_SAMPLES: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(1000));

// We'll avoid OnceCell and use a global atomic flag to track if the task is running
#[allow(dead_code)]
static METRICS_TASK_RUNNING: AtomicU64 = AtomicU64::new(0);

/// Initialize metric collection
#[allow(dead_code)]
pub fn init(interval_seconds: u64, max_samples: usize) {
    if interval_seconds > 0 {
        METRICS_ENABLED.store(1, Ordering::SeqCst);
        MAX_SAMPLES.store(max_samples as u64, Ordering::SeqCst);

        // Only start the task if it's not already running
        if METRICS_TASK_RUNNING
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            let query_durations: Arc<Mutex<Vec<Duration>>> = Arc::clone(&QUERY_DURATIONS);
            let mutation_durations: Arc<Mutex<Vec<Duration>>> = Arc::clone(&MUTATION_DURATIONS);

            // Spawn the task but we won't keep the handle
            let _handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));
                loop {
                    interval.tick().await;
                    compute_metric(query_durations.clone(), mutation_durations.clone());
                }
            });
        }
    } else {
        METRICS_ENABLED.store(0, Ordering::SeqCst);
    }
}

/// Record a query duration
pub fn record_query_duration(_duration: Duration) {
    // Disabled for now until we fix the metric issues
}

/// Record a mutation duration (create, update, delete)
pub fn record_mutation_duration(_duration: Duration) {
    // Disabled for now until we fix the metric issues
}

// Removed record_error function since it's not used

// Removed record_transaction_start function since it's not used

// Removed record_transaction_error function since it's not used

/// Compute and publish metric
#[allow(dead_code)]
fn compute_metric(
    _query_durations: Arc<Mutex<Vec<Duration>>>,
    _mutation_durations: Arc<Mutex<Vec<Duration>>>,
) {
    // Disabled for now until we fix the metric issues
}

// Removed get_metric_snapshot function since it's not used

// Removed unused MetricsSnapshot struct and its methods
