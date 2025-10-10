# CPUCACHE_1: Implement CPU Metrics Caching

**Status**: Ready for Implementation
**Priority**: HIGH
**Estimated Effort**: 2-3 days
**Package**: packages/surrealdb

---

## OBJECTIVE

Implement caching for CPU usage metrics to prevent thread pool exhaustion and performance degradation under high monitoring frequency.

---

## PROBLEM DESCRIPTION

The monitoring service uses `spawn_blocking` with `thread::sleep` for CPU metrics:

File: `packages/surrealdb/src/repository/monitoring_service.rs:322-332`
```rust
async fn get_current_cpu_usage(&self) -> Result<f64, RepositoryError> {
    let cpu_usage = tokio::task::spawn_blocking(|| {
        let mut system = System::new();
        system.refresh_cpu_all();

        std::thread::sleep(std::time::Duration::from_millis(200));
        system.refresh_cpu_all();

        system.global_cpu_usage() as f64
    }).await
    .map_err(|e| RepositoryError::SystemError(format!("CPU metrics task failed: {}", e)))?;
```

**Issues**:
- Blocks a tokio blocking thread for 200ms per call
- High-frequency monitoring exhausts the tokio blocking thread pool
- Under load, all blocking threads could be occupied sleeping
- No caching - every call requires a fresh 200ms measurement

**Impact**:
- Performance degradation under load
- Monitoring overhead becomes significant
- Potential task starvation for other blocking operations

---

## RESEARCH NOTES

**Why spawn_blocking is Necessary**:
- `sysinfo::System` API is synchronous/blocking
- `thread::sleep` is blocking (can't use `tokio::time::sleep` with blocking operations)
- Must use `spawn_blocking` to avoid blocking the async runtime

**Why We Can't Just Remove spawn_blocking**:
- The System API requires 200ms delay between refreshes for accurate CPU measurements
- This is a limitation of the underlying OS APIs
- We can't make it async without rewriting with different system metrics library

**Solution: Caching**:
- Cache CPU readings for a configurable interval (e.g., 5 seconds)
- Only spawn blocking task when cache is stale
- Reduces blocking thread usage by ~96% (5000ms / 200ms = 25x less frequent)

---

## SUBTASK 1: Create CPU Metrics Cache Structure

**Objective**: Define a thread-safe cache structure for CPU metrics.

**Location**: `packages/surrealdb/src/repository/monitoring_service.rs`

**Implementation**:

1. Add cache structure near the top of the file:
```rust
use tokio::sync::RwLock;
use std::time::Instant;

/// Cache for CPU usage metrics to avoid excessive spawn_blocking calls
struct CpuMetricsCache {
    /// Last CPU reading with timestamp
    last_reading: Arc<RwLock<Option<CachedCpuReading>>>,

    /// How long cached values remain valid
    cache_duration: Duration,
}

#[derive(Clone)]
struct CachedCpuReading {
    /// CPU usage percentage (0.0 - 100.0)
    value: f64,

    /// When this reading was taken
    timestamp: Instant,
}

impl CpuMetricsCache {
    /// Create a new cache with the specified duration
    fn new(cache_duration: Duration) -> Self {
        Self {
            last_reading: Arc::new(RwLock::new(None)),
            cache_duration,
        }
    }

    /// Get cached value if still valid
    async fn get(&self) -> Option<f64> {
        let cache = self.last_reading.read().await;

        if let Some(reading) = cache.as_ref() {
            if reading.timestamp.elapsed() < self.cache_duration {
                tracing::trace!(
                    "CPU cache hit (age: {:?})",
                    reading.timestamp.elapsed()
                );
                return Some(reading.value);
            } else {
                tracing::trace!(
                    "CPU cache expired (age: {:?})",
                    reading.timestamp.elapsed()
                );
            }
        }

        None
    }

    /// Store a new reading in the cache
    async fn set(&self, value: f64) {
        let mut cache = self.last_reading.write().await;
        *cache = Some(CachedCpuReading {
            value,
            timestamp: Instant::now(),
        });
        tracing::trace!("CPU cache updated: {:.2}%", value);
    }
}
```

**Files to Modify**:
- `packages/surrealdb/src/repository/monitoring_service.rs`

**Definition of Done**:
- CpuMetricsCache struct defined
- Thread-safe using RwLock
- get() method checks cache validity
- set() method updates cache with timestamp
- Logging for cache hits/misses

---

## SUBTASK 2: Add Cache Field to MonitoringService

**Objective**: Integrate the cache into the MonitoringService struct.

**Location**: `packages/surrealdb/src/repository/monitoring_service.rs`

**Changes Required**:

1. Add cache field to MonitoringService struct:
```rust
pub struct MonitoringService {
    db: Surreal<Any>,

    /// Cache for CPU metrics to reduce spawn_blocking overhead
    cpu_cache: CpuMetricsCache,

    // ... existing fields
}
```

2. Update constructor:
```rust
impl MonitoringService {
    pub fn new(db: Surreal<Any>) -> Self {
        Self {
            db,
            cpu_cache: CpuMetricsCache::new(Duration::from_secs(5)),
            // ... existing field initialization
        }
    }

    /// Create with custom cache duration (useful for different monitoring frequencies)
    pub fn with_cache_duration(db: Surreal<Any>, cache_duration: Duration) -> Self {
        Self {
            db,
            cpu_cache: CpuMetricsCache::new(cache_duration),
            // ... existing field initialization
        }
    }
}
```

**Files to Modify**:
- `packages/surrealdb/src/repository/monitoring_service.rs`

**Definition of Done**:
- cpu_cache field added to struct
- Constructor initializes cache with 5-second default
- Alternative constructor for custom cache duration
- Documentation explains the caching behavior

---

## SUBTASK 3: Update get_current_cpu_usage to Use Cache

**Objective**: Modify the CPU usage method to check cache before spawning blocking task.

**Location**: `packages/surrealdb/src/repository/monitoring_service.rs` (around line 322)

**Current Code**:
```rust
async fn get_current_cpu_usage(&self) -> Result<f64, RepositoryError> {
    let cpu_usage = tokio::task::spawn_blocking(|| {
        let mut system = System::new();
        system.refresh_cpu_all();

        std::thread::sleep(std::time::Duration::from_millis(200));
        system.refresh_cpu_all();

        system.global_cpu_usage() as f64
    }).await
    .map_err(|e| RepositoryError::SystemError(format!("CPU metrics task failed: {}", e)))?;
```

**Updated Implementation**:
```rust
/// Get current CPU usage percentage
///
/// Uses a 5-second cache to avoid excessive spawn_blocking calls.
/// CPU measurements require a 200ms delay, so high-frequency calls
/// would exhaust the tokio blocking thread pool without caching.
///
/// # Returns
/// CPU usage as a percentage (0.0 - 100.0)
async fn get_current_cpu_usage(&self) -> Result<f64, RepositoryError> {
    // Check cache first
    if let Some(cached_value) = self.cpu_cache.get().await {
        return Ok(cached_value);
    }

    // Cache miss - need fresh measurement
    tracing::debug!("CPU cache miss, spawning blocking task for fresh measurement");

    let cpu_usage = tokio::task::spawn_blocking(|| {
        let mut system = System::new();
        system.refresh_cpu_all();

        // Required delay for accurate CPU measurement
        std::thread::sleep(std::time::Duration::from_millis(200));
        system.refresh_cpu_all();

        system.global_cpu_usage() as f64
    })
    .await
    .map_err(|e| RepositoryError::SystemError(format!("CPU metrics task failed: {}", e)))?;

    // Update cache with fresh value
    self.cpu_cache.set(cpu_usage).await;

    Ok(cpu_usage)
}
```

**Files to Modify**:
- `packages/surrealdb/src/repository/monitoring_service.rs` (lines 322-332)

**Definition of Done**:
- Method checks cache before spawning task
- Cache miss triggers fresh measurement
- Fresh measurements update the cache
- Documentation explains caching behavior
- Logging for cache hits/misses
- No unwrap() or expect() calls

---

## SUBTASK 4: Add Cache Invalidation Method (Optional)

**Objective**: Allow manual cache invalidation if needed.

**Location**: `packages/surrealdb/src/repository/monitoring_service.rs`

**Implementation**:

Add method to MonitoringService:
```rust
impl MonitoringService {
    /// Invalidate the CPU metrics cache
    ///
    /// Forces the next call to get_current_cpu_usage() to fetch a fresh
    /// measurement. Useful for testing or when immediate accuracy is required.
    pub async fn invalidate_cpu_cache(&self) {
        let mut cache = self.cpu_cache.last_reading.write().await;
        *cache = None;
        tracing::debug!("CPU cache invalidated");
    }

    /// Get the age of the current cached CPU value
    ///
    /// Returns None if no cached value exists.
    pub async fn cpu_cache_age(&self) -> Option<Duration> {
        let cache = self.cpu_cache.last_reading.read().await;
        cache.as_ref().map(|reading| reading.timestamp.elapsed())
    }
}
```

**Files to Modify**:
- `packages/surrealdb/src/repository/monitoring_service.rs`

**Definition of Done**:
- invalidate_cpu_cache() method clears the cache
- cpu_cache_age() method returns cache age
- Methods are properly documented
- Logging for invalidation

---

## SUBTASK 5: Update Similar Memory and Disk Metrics (If Applicable)

**Objective**: Apply similar caching to memory and disk metrics if they also use spawn_blocking.

**Location**: `packages/surrealdb/src/repository/monitoring_service.rs`

**Check These Methods**:
- `get_current_memory_usage()`
- `get_current_disk_usage()`
- Any other system metrics methods

**If They Use spawn_blocking**:
1. Add similar cache structures (MemoryMetricsCache, DiskMetricsCache)
2. Update methods to use cache-then-fetch pattern
3. Use appropriate cache durations:
   - Memory: 5 seconds (changes frequently)
   - Disk: 30 seconds (changes slowly)

**Implementation Pattern** (same as CPU):
```rust
async fn get_current_memory_usage(&self) -> Result<MemoryMetrics, RepositoryError> {
    // Check cache first
    if let Some(cached) = self.memory_cache.get().await {
        return Ok(cached);
    }

    // Cache miss - fetch fresh data
    let metrics = tokio::task::spawn_blocking(|| {
        let mut system = System::new();
        system.refresh_memory();

        MemoryMetrics {
            used: system.used_memory(),
            total: system.total_memory(),
        }
    }).await?;

    // Update cache
    self.memory_cache.set(metrics.clone()).await;

    Ok(metrics)
}
```

**Files to Modify**:
- `packages/surrealdb/src/repository/monitoring_service.rs` (other metric methods)

**Definition of Done**:
- All spawn_blocking system metrics use caching
- Appropriate cache durations for each metric type
- Consistent pattern across all metrics

---

## CONSTRAINTS

⚠️ **NO TESTS**: Do not write unit tests, integration tests, or test fixtures. Test team handles all testing.

⚠️ **NO BENCHMARKS**: Do not write benchmark code. Performance team handles benchmarking.

⚠️ **FOCUS ON FUNCTIONALITY**: Only modify production code in ./src directories.

---

## DEPENDENCIES

**Rust Crates** (likely already in Cargo.toml):
- tokio (with sync feature for RwLock)
- sysinfo (existing dependency)

**Design Considerations**:
- Cannot use async system metrics library (sysinfo is the standard)
- Must keep spawn_blocking for blocking System API
- Cache is the only viable solution without major refactoring

---

## DEFINITION OF DONE

- [ ] CpuMetricsCache struct implemented with get/set methods
- [ ] MonitoringService has cpu_cache field
- [ ] get_current_cpu_usage() checks cache before spawning blocking task
- [ ] Fresh measurements update the cache
- [ ] Cache duration configurable (default 5 seconds)
- [ ] Logging for cache hits, misses, and updates
- [ ] Manual cache invalidation method added
- [ ] Similar caching applied to memory/disk metrics if applicable
- [ ] No compilation errors
- [ ] No test code written
- [ ] No benchmark code written

---

## FILES TO MODIFY

1. `packages/surrealdb/src/repository/monitoring_service.rs` (lines 322-332 + new cache code)

---

## NOTES

- Cache duration of 5 seconds reduces blocking thread usage by ~96%
- This is the correct approach - spawn_blocking is necessary for sysinfo API
- Alternative would be using a different system metrics library (much larger change)
- RwLock is appropriate here (many readers, infrequent writers)
- Cache should be transparent to callers - same API surface
- Consider making cache duration configurable via environment variable
- Trace-level logging for cache operations to avoid log spam
