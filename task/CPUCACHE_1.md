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

The monitoring service uses `spawn_blocking` with `thread::sleep` for CPU metrics collection:

**File**: [`packages/surrealdb/src/repository/monitoring_service.rs:322-334`](../packages/surrealdb/src/repository/monitoring_service.rs)

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

    Ok(cpu_usage)
}
```

### Core Issues

1. **Blocks tokio blocking thread for 200ms per call**
2. **High-frequency monitoring exhausts the blocking thread pool**
3. **No caching - every call requires a fresh 200ms measurement**
4. **Thread starvation** for other blocking operations under load

### Performance Impact

- Monitoring overhead becomes significant under load
- Performance degradation when all blocking threads are sleeping
- Potential task starvation for other blocking operations
- Response time increases for monitoring endpoints

---

## RESEARCH: Why the 200ms Delay is Mandatory

### OS-Level Requirement

The 200ms delay is **not** a sysinfo library limitation—it's an **OS-level requirement** for accurate CPU measurements.

**Source**: [`tmp/sysinfo/src/unix/apple/system.rs`](../tmp/sysinfo/src/unix/apple/system.rs)
```rust
pub const MINIMUM_CPU_UPDATE_INTERVAL: Duration = Duration::from_millis(200);
```

**Platform-specific values**:
- **macOS**: 200ms ([`tmp/sysinfo/src/unix/apple/system.rs`](../tmp/sysinfo/src/unix/apple/system.rs))
- **Linux**: 200ms ([`tmp/sysinfo/src/unix/linux/system.rs`](../tmp/sysinfo/src/unix/linux/system.rs))
- **Windows**: 200ms ([`tmp/sysinfo/src/windows/system.rs`](../tmp/sysinfo/src/windows/system.rs))
- **FreeBSD**: 100ms ([`tmp/sysinfo/src/unix/freebsd/system.rs`](../tmp/sysinfo/src/unix/freebsd/system.rs))

**From sysinfo documentation** ([`tmp/sysinfo/md_doc/minimum_cpu_update_interval.md`](../tmp/sysinfo/md_doc/minimum_cpu_update_interval.md)):

> This is the minimum interval time used internally by `sysinfo` to refresh the CPU time.
> 
> ⚠️ This value differs from one OS to another.
> 
> Why is this constant even needed?
> 
> If refreshed too often, the CPU usage of processes will be `0` whereas on Linux it'll always be the maximum value (`number of CPUs * 100`).

### How CPU Usage is Measured

CPU usage is calculated as a **delta between two time points**:

1. **First measurement**: Capture CPU counters at time T₀
2. **Wait period**: Allow CPU to do work (200ms minimum)
3. **Second measurement**: Capture CPU counters at time T₁
4. **Calculation**: `usage = (counters_T1 - counters_T0) / elapsed_time`

**This is why**:
- We cannot make the measurement async (blocking operation is fundamental)
- We cannot remove the sleep (OS requires time delta for accuracy)
- We cannot speed it up (OS limitation, not library limitation)
- **Caching is the only viable solution**

**Reference**: [`tmp/sysinfo/README.md`](../tmp/sysinfo/README.md) lines 109-122

---

## EXISTING CACHING PATTERN IN THE CODEBASE

The `MonitoringService` **already implements caching** for network metrics using a similar pattern:

**File**: [`packages/surrealdb/src/repository/monitoring_service.rs:22-28`](../packages/surrealdb/src/repository/monitoring_service.rs)

```rust
#[derive(Clone)]
pub struct MonitoringService<C: Connection> {
    metrics_repo: MetricsRepository<C>,
    performance_repo: PerformanceRepository<C>,
    monitoring_repo: MonitoringRepository<C>,
    last_network_stats: Arc<Mutex<Option<(Instant, u64, u64)>>>,  // <-- Existing cache!
}
```

**Network caching implementation** (lines 393-425):

```rust
async fn get_network_throughput(&self) -> Result<f64, RepositoryError> {
    let last_stats = self.last_network_stats.clone();

    let throughput = tokio::task::spawn_blocking(move || {
        let networks = Networks::new_with_refreshed_list();
        let now = Instant::now();
        
        // ... network stats collection ...
        
        let mut last = last_stats.blocking_lock();  // Uses Mutex
        let bytes_per_sec = if let Some((prev_time, prev_bytes, _)) = *last {
            let duration_secs = now.duration_since(prev_time).as_secs_f64();
            if duration_secs > 0.0 {
                (total_bytes.saturating_sub(prev_bytes)) as f64 / duration_secs
            } else {
                0.0
            }
        } else {
            0.0
        };

        *last = Some((now, total_bytes, total_tx));  // Update cache
        bytes_per_sec
    }).await?;

    Ok(throughput)
}
```

**Pattern to follow**:
- Store cached value with timestamp in the struct
- Check cache validity before expensive operation
- Update cache after fresh measurement

**Why our CPU cache will be different**:
- Network cache uses `Mutex` (locks for both reads and writes)
- CPU cache will use `RwLock` (multiple concurrent readers, single writer)
- Network computes deltas (more complex logic)
- CPU is simple value caching (simpler, clearer design)

---

## ARCHITECTURE DECISION: RwLock vs Mutex vs moka

### Why RwLock?

**Read-heavy workload characteristics**:
- Monitoring endpoints check CPU usage frequently (potentially 100s-1000s times per second)
- Cache writes only happen every ~5 seconds (when cache expires)
- Read-to-write ratio: ~500:1 or higher

**RwLock advantages**:
- ✅ Multiple concurrent readers (no contention for cached reads)
- ✅ Single writer when cache expires
- ✅ Perfect for high-frequency reads with infrequent updates
- ✅ Significantly better performance for this access pattern

**Mutex disadvantages**:
- ❌ Only one accessor at a time (reader OR writer)
- ❌ All monitoring calls queue up even for cached reads
- ❌ Creates bottleneck under high load
- ❌ The existing network cache uses Mutex (less optimal)

### Why Not moka::future::Cache?

**moka** is already a dependency ([`packages/surrealdb/Cargo.toml:27`](../packages/surrealdb/Cargo.toml)):
```toml
moka = { version = "0.12.11", features = ["future"] }
```

**Used in the codebase** ([`packages/surrealdb/src/repository/membership.rs:6`](../packages/surrealdb/src/repository/membership.rs)):
```rust
use moka::future::Cache;

// Used for key-value caching (room creators)
pub async fn get_room_creator_cached(
    &self,
    room_id: &str,
    cache: &Cache<String, Option<String>>,  // <-- moka for K-V pairs
) -> Result<Option<String>, RepositoryError>
```

**Why moka is not appropriate here**:
- ❌ **Overkill**: Designed for key-value caching, we have single-value caching
- ❌ **Less explicit**: TTL-based eviction vs explicit timestamp-based validity
- ❌ **More complex**: Additional dependency overhead for simple use case
- ✅ **RwLock is simpler**: More explicit about caching semantics and timing
- ✅ **RwLock is lighter**: Less memory overhead for single-value cache

**When to use moka**: Multiple keys (e.g., caching CPU per process ID)  
**When to use RwLock**: Single cached value with time-based expiration

---

## SPAWN_BLOCKING USAGE ANALYSIS

All system metrics in `MonitoringService` use `spawn_blocking`:

| Method | Lines | Sleep Duration | Current Caching | Cache Priority |
|--------|-------|----------------|-----------------|----------------|
| `get_current_cpu_usage()` | 322-334 | **200ms** | ❌ None | 🔴 **CRITICAL** |
| `get_current_memory_usage()` | 336-346 | None | ❌ None | 🟡 **MEDIUM** |
| `get_current_disk_usage()` | 348-391 | None | ❌ None | 🟡 **MEDIUM** |
| `get_network_throughput()` | 393-425 | None | ✅ Mutex cache | ✅ **Already cached** |

**Recommended cache durations**:
- **CPU**: 5 seconds (high change frequency, 200ms measurement cost)
- **Memory**: 5 seconds (moderate change frequency)
- **Disk**: 30 seconds (low change frequency, changes slowly)
- **Network**: Custom delta-based (already implemented)

---

## SOLUTION OVERVIEW

**Strategy**: Implement time-based caching with `Arc<RwLock<Option<CachedReading>>>` pattern

**Benefits**:
- ✅ Reduces blocking thread usage by ~96% (5000ms / 200ms = 25x reduction)
- ✅ Allows concurrent reads for cached values (RwLock)
- ✅ Maintains accurate readings (5-second staleness is acceptable for monitoring)
- ✅ Prevents thread pool exhaustion
- ✅ Transparent to callers (same API surface)

**Cache behavior**:
- ⏱️ **Cache hit** (age < 5s): Return cached value immediately (no blocking)
- ⏱️ **Cache miss** (age ≥ 5s): Spawn blocking task, update cache, return fresh value
- 🔒 **Concurrent reads**: Multiple callers can read cached value simultaneously
- 📝 **Single writer**: Only one task updates cache when expired

---

## IMPLEMENTATION GUIDE

### SUBTASK 1: Create CPU Metrics Cache Structure

**Objective**: Define thread-safe cache structure for CPU metrics.

**Location**: [`packages/surrealdb/src/repository/monitoring_service.rs`](../packages/surrealdb/src/repository/monitoring_service.rs) (top of file, after imports)

**Add imports**:
```rust
use std::time::Instant;  // Already imported
use tokio::sync::RwLock;  // ADD THIS
```

**Add cache structures** (insert after imports, before `MonitoringService` struct):

```rust
/// Cached CPU reading with timestamp for time-based expiration
#[derive(Clone)]
struct CachedCpuReading {
    /// CPU usage percentage (0.0 - 100.0)
    value: f64,
    
    /// When this reading was captured
    timestamp: Instant,
}

/// Thread-safe cache for CPU usage metrics
/// 
/// Uses RwLock to allow concurrent reads of cached values while
/// serializing writes when the cache expires and needs updating.
/// 
/// Design rationale:
/// - High read frequency (100s-1000s of monitoring calls per second)
/// - Low write frequency (once every ~5 seconds when cache expires)
/// - RwLock provides better performance than Mutex for this access pattern
struct CpuMetricsCache {
    /// Last CPU reading with timestamp
    last_reading: Arc<RwLock<Option<CachedCpuReading>>>,
    
    /// How long cached values remain valid
    cache_duration: std::time::Duration,
}

impl CpuMetricsCache {
    /// Create a new cache with specified duration
    fn new(cache_duration: std::time::Duration) -> Self {
        Self {
            last_reading: Arc::new(RwLock::new(None)),
            cache_duration,
        }
    }
    
    /// Get cached value if still valid
    /// 
    /// Returns Some(value) if cache hit, None if cache miss or expired.
    /// Uses read lock for concurrent access by multiple callers.
    async fn get(&self) -> Option<f64> {
        let cache = self.last_reading.read().await;
        
        if let Some(reading) = cache.as_ref() {
            if reading.timestamp.elapsed() < self.cache_duration {
                tracing::trace!(
                    cpu_usage = reading.value,
                    cache_age_ms = reading.timestamp.elapsed().as_millis(),
                    "CPU cache hit"
                );
                return Some(reading.value);
            } else {
                tracing::trace!(
                    cache_age_ms = reading.timestamp.elapsed().as_millis(),
                    cache_duration_ms = self.cache_duration.as_millis(),
                    "CPU cache expired"
                );
            }
        }
        
        None
    }
    
    /// Store a new reading in the cache
    /// 
    /// Uses write lock to update cache atomically.
    /// Only one writer can update at a time.
    async fn set(&self, value: f64) {
        let mut cache = self.last_reading.write().await;
        *cache = Some(CachedCpuReading {
            value,
            timestamp: Instant::now(),
        });
        
        tracing::trace!(
            cpu_usage = value,
            "CPU cache updated with fresh measurement"
        );
    }
}
```

**Files Modified**:
- [`packages/surrealdb/src/repository/monitoring_service.rs`](../packages/surrealdb/src/repository/monitoring_service.rs)

**Definition of Done**:
- ✅ `CachedCpuReading` struct with value and timestamp
- ✅ `CpuMetricsCache` struct with RwLock-protected cache
- ✅ `get()` method checks cache validity using read lock
- ✅ `set()` method updates cache using write lock
- ✅ Trace-level logging for cache hits/misses
- ✅ Documentation explaining RwLock choice

---

### SUBTASK 2: Add Cache Field to MonitoringService

**Objective**: Integrate cache into the `MonitoringService` struct.

**Location**: [`packages/surrealdb/src/repository/monitoring_service.rs:22-28`](../packages/surrealdb/src/repository/monitoring_service.rs)

**Current struct**:
```rust
#[derive(Clone)]
pub struct MonitoringService<C: Connection> {
    metrics_repo: MetricsRepository<C>,
    performance_repo: PerformanceRepository<C>,
    monitoring_repo: MonitoringRepository<C>,
    last_network_stats: Arc<Mutex<Option<(Instant, u64, u64)>>>,
}
```

**Updated struct**:
```rust
#[derive(Clone)]
pub struct MonitoringService<C: Connection> {
    metrics_repo: MetricsRepository<C>,
    performance_repo: PerformanceRepository<C>,
    monitoring_repo: MonitoringRepository<C>,
    last_network_stats: Arc<Mutex<Option<(Instant, u64, u64)>>>,
    
    /// Cache for CPU metrics to reduce spawn_blocking overhead
    /// 
    /// Caches CPU readings for 5 seconds to prevent thread pool exhaustion.
    /// See CPUCACHE_1 task for rationale and implementation details.
    cpu_cache: CpuMetricsCache,
}
```

**Update constructor** (lines 30-37):

```rust
impl<C: Connection> MonitoringService<C> {
    pub fn new(db: Surreal<C>) -> Self {
        Self {
            metrics_repo: MetricsRepository::new(db.clone()),
            performance_repo: PerformanceRepository::new(db.clone()),
            monitoring_repo: MonitoringRepository::new(db),
            last_network_stats: Arc::new(Mutex::new(None)),
            cpu_cache: CpuMetricsCache::new(std::time::Duration::from_secs(5)),  // ADD THIS
        }
    }
    
    /// Create monitoring service with custom CPU cache duration
    /// 
    /// Useful for different monitoring frequencies or testing scenarios.
    /// Default duration is 5 seconds (see `new()`).
    /// 
    /// # Arguments
    /// * `db` - SurrealDB connection
    /// * `cpu_cache_duration` - How long CPU readings remain valid
    pub fn with_cpu_cache_duration(db: Surreal<C>, cpu_cache_duration: std::time::Duration) -> Self {
        Self {
            metrics_repo: MetricsRepository::new(db.clone()),
            performance_repo: PerformanceRepository::new(db.clone()),
            monitoring_repo: MonitoringRepository::new(db),
            last_network_stats: Arc::new(Mutex::new(None)),
            cpu_cache: CpuMetricsCache::new(cpu_cache_duration),
        }
    }
}
```

**Files Modified**:
- [`packages/surrealdb/src/repository/monitoring_service.rs`](../packages/surrealdb/src/repository/monitoring_service.rs) (lines 22-37)

**Definition of Done**:
- ✅ `cpu_cache` field added to struct
- ✅ Constructor initializes cache with 5-second default
- ✅ Alternative constructor `with_cpu_cache_duration()` for custom duration
- ✅ Documentation explains caching behavior

---

### SUBTASK 3: Update get_current_cpu_usage to Use Cache

**Objective**: Modify CPU usage method to check cache before spawning blocking task.

**Location**: [`packages/surrealdb/src/repository/monitoring_service.rs:322-334`](../packages/surrealdb/src/repository/monitoring_service.rs)

**Current implementation**:
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

    Ok(cpu_usage)
}
```

**Updated implementation with caching**:
```rust
/// Get current CPU usage percentage
///
/// Uses a 5-second cache to avoid excessive spawn_blocking calls.
/// CPU measurements require a 200ms delay between readings due to OS-level
/// requirements (see sysinfo::MINIMUM_CPU_UPDATE_INTERVAL). Without caching,
/// high-frequency monitoring would exhaust the tokio blocking thread pool.
///
/// # Caching Behavior
/// - Cache hit (< 5s old): Returns immediately from cache (no blocking)
/// - Cache miss (≥ 5s old): Spawns blocking task, updates cache, returns fresh value
///
/// # Returns
/// CPU usage as a percentage (0.0 - 100.0)
///
/// # Errors
/// Returns `RepositoryError::SystemError` if the blocking task fails
async fn get_current_cpu_usage(&self) -> Result<f64, RepositoryError> {
    // Check cache first (fast path - uses read lock, allows concurrent access)
    if let Some(cached_value) = self.cpu_cache.get().await {
        return Ok(cached_value);
    }

    // Cache miss - need fresh measurement (slow path - 200ms blocking operation)
    tracing::debug!("CPU cache miss, spawning blocking task for fresh measurement");

    let cpu_usage = tokio::task::spawn_blocking(|| {
        let mut system = System::new();
        system.refresh_cpu_all();

        // Required delay for accurate CPU measurement (OS requirement)
        // See: tmp/sysinfo/src/unix/apple/system.rs - MINIMUM_CPU_UPDATE_INTERVAL
        std::thread::sleep(std::time::Duration::from_millis(200));
        system.refresh_cpu_all();

        system.global_cpu_usage() as f64
    })
    .await
    .map_err(|e| RepositoryError::SystemError(format!("CPU metrics task failed: {}", e)))?;

    // Update cache with fresh value (uses write lock, single writer)
    self.cpu_cache.set(cpu_usage).await;

    Ok(cpu_usage)
}
```

**Files Modified**:
- [`packages/surrealdb/src/repository/monitoring_service.rs`](../packages/surrealdb/src/repository/monitoring_service.rs) (lines 322-334)

**Definition of Done**:
- ✅ Method checks cache before spawning task
- ✅ Cache miss triggers fresh measurement
- ✅ Fresh measurements update the cache
- ✅ Comprehensive documentation explaining behavior
- ✅ Debug-level logging for cache misses
- ✅ Trace-level logging for cache hits (in cache.get())

---

### SUBTASK 4: Add Cache Management Methods

**Objective**: Provide manual cache control for special cases.

**Location**: [`packages/surrealdb/src/repository/monitoring_service.rs`](../packages/surrealdb/src/repository/monitoring_service.rs) (add to `impl<C: Connection> MonitoringService<C>` block)

**Add public methods**:

```rust
impl<C: Connection> MonitoringService<C> {
    // ... existing methods ...
    
    /// Invalidate the CPU metrics cache
    ///
    /// Forces the next call to `get_current_cpu_usage()` to fetch a fresh
    /// measurement regardless of cache age. Useful for:
    /// - Forcing immediate refresh after system changes
    /// - Clearing stale data after long idle periods
    ///
    /// # Example
    /// ```rust,no_run
    /// # async fn example(service: &MonitoringService) {
    /// // Clear cache before critical measurement
    /// service.invalidate_cpu_cache().await;
    /// let fresh_cpu = service.get_current_cpu_usage().await?;
    /// # }
    /// ```
    pub async fn invalidate_cpu_cache(&self) {
        let mut cache = self.cpu_cache.last_reading.write().await;
        *cache = None;
        tracing::debug!("CPU cache manually invalidated");
    }

    /// Get the age of the current cached CPU value
    ///
    /// Returns `None` if no cached value exists, otherwise returns the
    /// duration since the value was cached.
    ///
    /// Useful for monitoring and diagnostics.
    ///
    /// # Example
    /// ```rust,no_run
    /// # async fn example(service: &MonitoringService) {
    /// if let Some(age) = service.cpu_cache_age().await {
    ///     println!("CPU cache is {} seconds old", age.as_secs());
    /// } else {
    ///     println!("No CPU value cached");
    /// }
    /// # }
    /// ```
    pub async fn cpu_cache_age(&self) -> Option<std::time::Duration> {
        let cache = self.cpu_cache.last_reading.read().await;
        cache.as_ref().map(|reading| reading.timestamp.elapsed())
    }
    
    /// Check if CPU cache is currently valid (not expired)
    ///
    /// Returns `true` if a cached value exists and is within the cache duration.
    ///
    /// # Example
    /// ```rust,no_run
    /// # async fn example(service: &MonitoringService) {
    /// if service.is_cpu_cache_valid().await {
    ///     // Next CPU read will use cache (fast)
    /// } else {
    ///     // Next CPU read will spawn blocking task (slow)
    /// }
    /// # }
    /// ```
    pub async fn is_cpu_cache_valid(&self) -> bool {
        self.cpu_cache.get().await.is_some()
    }
}
```

**Files Modified**:
- [`packages/surrealdb/src/repository/monitoring_service.rs`](../packages/surrealdb/src/repository/monitoring_service.rs)

**Definition of Done**:
- ✅ `invalidate_cpu_cache()` method clears cache
- ✅ `cpu_cache_age()` method returns cache age
- ✅ `is_cpu_cache_valid()` method checks cache validity
- ✅ Methods properly documented with examples
- ✅ Logging for invalidation events

---

### SUBTASK 5: Apply Caching to Memory and Disk Metrics

**Objective**: Extend caching pattern to other spawn_blocking system metrics.

**Location**: [`packages/surrealdb/src/repository/monitoring_service.rs`](../packages/surrealdb/src/repository/monitoring_service.rs)

**Methods to update**:
- `get_current_memory_usage()` (lines 336-346) - 5 second cache
- `get_current_disk_usage()` (lines 348-391) - 30 second cache

**Pattern**: Same as CPU cache but with different durations

**1. Add cache structures** (after `CpuMetricsCache`):

```rust
/// Cached memory metrics
#[derive(Clone)]
struct CachedMemoryReading {
    value: f64,  // Memory in MB
    timestamp: Instant,
}

/// Cached disk metrics  
#[derive(Clone)]
struct CachedDiskReading {
    value: f64,  // Disk usage in MB
    timestamp: Instant,
}

/// Memory metrics cache (5 second duration - changes moderately)
struct MemoryMetricsCache {
    last_reading: Arc<RwLock<Option<CachedMemoryReading>>>,
    cache_duration: std::time::Duration,
}

impl MemoryMetricsCache {
    fn new(cache_duration: std::time::Duration) -> Self {
        Self {
            last_reading: Arc::new(RwLock::new(None)),
            cache_duration,
        }
    }
    
    async fn get(&self) -> Option<f64> {
        let cache = self.last_reading.read().await;
        if let Some(reading) = cache.as_ref() {
            if reading.timestamp.elapsed() < self.cache_duration {
                tracing::trace!(memory_mb = reading.value, "Memory cache hit");
                return Some(reading.value);
            }
        }
        None
    }
    
    async fn set(&self, value: f64) {
        let mut cache = self.last_reading.write().await;
        *cache = Some(CachedMemoryReading {
            value,
            timestamp: Instant::now(),
        });
        tracing::trace!(memory_mb = value, "Memory cache updated");
    }
}

/// Disk metrics cache (30 second duration - changes slowly)
struct DiskMetricsCache {
    last_reading: Arc<RwLock<Option<CachedDiskReading>>>,
    cache_duration: std::time::Duration,
}

impl DiskMetricsCache {
    fn new(cache_duration: std::time::Duration) -> Self {
        Self {
            last_reading: Arc::new(RwLock::new(None)),
            cache_duration,
        }
    }
    
    async fn get(&self) -> Option<f64> {
        let cache = self.last_reading.read().await;
        if let Some(reading) = cache.as_ref() {
            if reading.timestamp.elapsed() < self.cache_duration {
                tracing::trace!(disk_mb = reading.value, "Disk cache hit");
                return Some(reading.value);
            }
        }
        None
    }
    
    async fn set(&self, value: f64) {
        let mut cache = self.last_reading.write().await;
        *cache = Some(CachedDiskReading {
            value,
            timestamp: Instant::now(),
        });
        tracing::trace!(disk_mb = value, "Disk cache updated");
    }
}
```

**2. Add cache fields to MonitoringService**:

```rust
#[derive(Clone)]
pub struct MonitoringService<C: Connection> {
    metrics_repo: MetricsRepository<C>,
    performance_repo: PerformanceRepository<C>,
    monitoring_repo: MonitoringRepository<C>,
    last_network_stats: Arc<Mutex<Option<(Instant, u64, u64)>>>,
    cpu_cache: CpuMetricsCache,
    memory_cache: MemoryMetricsCache,  // ADD THIS
    disk_cache: DiskMetricsCache,      // ADD THIS
}
```

**3. Update constructor**:

```rust
pub fn new(db: Surreal<C>) -> Self {
    Self {
        metrics_repo: MetricsRepository::new(db.clone()),
        performance_repo: PerformanceRepository::new(db.clone()),
        monitoring_repo: MonitoringRepository::new(db),
        last_network_stats: Arc::new(Mutex::new(None)),
        cpu_cache: CpuMetricsCache::new(std::time::Duration::from_secs(5)),
        memory_cache: MemoryMetricsCache::new(std::time::Duration::from_secs(5)),   // ADD
        disk_cache: DiskMetricsCache::new(std::time::Duration::from_secs(30)),      // ADD
    }
}
```

**4. Update get_current_memory_usage()** (lines 336-346):

```rust
async fn get_current_memory_usage(&self) -> Result<f64, RepositoryError> {
    // Check cache first
    if let Some(cached_value) = self.memory_cache.get().await {
        return Ok(cached_value);
    }

    // Cache miss - fetch fresh data
    let memory_mb = tokio::task::spawn_blocking(|| {
        let mut system = System::new();
        system.refresh_memory();

        (system.used_memory() / 1024 / 1024) as f64
    }).await
    .map_err(|e| RepositoryError::SystemError(format!("Memory metrics task failed: {}", e)))?;

    // Update cache
    self.memory_cache.set(memory_mb).await;

    Ok(memory_mb)
}
```

**5. Update get_current_disk_usage()** (lines 348-391):

```rust
async fn get_current_disk_usage(&self) -> Result<f64, RepositoryError> {
    // Check cache first
    if let Some(cached_value) = self.disk_cache.get().await {
        return Ok(cached_value);
    }

    // Cache miss - fetch fresh data
    let data_path = std::env::current_dir()
        .map_err(|e| RepositoryError::SystemError(format!("Failed to get current dir: {}", e)))?;

    let disk_usage_mb = tokio::task::spawn_blocking(move || {
        let disks = Disks::new_with_refreshed_list();

        for disk in disks.list() {
            if let Some(mount_point) = disk.mount_point().to_str()
                && data_path.starts_with(mount_point)
            {
                let total_mb = (disk.total_space() / 1024 / 1024) as f64;
                let available_mb = (disk.available_space() / 1024 / 1024) as f64;
                let used_mb = total_mb - available_mb;
                return used_mb;
            }
        }

        disks.list().iter()
            .map(|disk| {
                let total = disk.total_space();
                let available = disk.available_space();
                ((total - available) / 1024 / 1024) as f64
            })
            .sum()
    }).await
    .map_err(|e| RepositoryError::SystemError(format!("Disk metrics task failed: {}", e)))?;

    // Update cache
    self.disk_cache.set(disk_usage_mb).await;

    Ok(disk_usage_mb)
}
```

**Files Modified**:
- [`packages/surrealdb/src/repository/monitoring_service.rs`](../packages/surrealdb/src/repository/monitoring_service.rs) (multiple sections)

**Definition of Done**:
- ✅ Memory cache with 5-second duration
- ✅ Disk cache with 30-second duration
- ✅ Both use same RwLock pattern as CPU
- ✅ Cache-check-then-fetch pattern applied to both methods
- ✅ Consistent logging across all cached metrics

---

## DEPENDENCIES

**Existing dependencies** ([`packages/surrealdb/Cargo.toml`](../packages/surrealdb/Cargo.toml)):

```toml
tokio = { version = "1", features = ["full"] }  # Includes "sync" feature for RwLock
sysinfo = "0.37"                                 # System metrics library
```

**No new dependencies required** - all needed crates already present.

---

## TECHNICAL NOTES

### sysinfo Library Behavior

**CPU measurement is NOT instantaneous**:
- First `refresh_cpu_all()`: Captures CPU counters at T₀
- `sleep(200ms)`: Allow CPU to do work
- Second `refresh_cpu_all()`: Captures CPU counters at T₁  
- `global_cpu_usage()`: Returns `(T₁ - T₀) / 200ms`

**Reference**: [`tmp/sysinfo/README.md`](../tmp/sysinfo/README.md) lines 109-122

### Why We Cannot Eliminate spawn_blocking

1. **sysinfo API is synchronous** - No async alternative exists
2. **thread::sleep is blocking** - Cannot use tokio::time::sleep in spawn_blocking
3. **OS requires time delta** - Cannot measure CPU usage instantaneously
4. **Alternative would require different library** - Major refactoring

**Caching is the only viable solution without major architectural changes.**

### Cache Duration Rationale

| Metric | Duration | Rationale |
|--------|----------|-----------|
| CPU | 5s | Changes quickly, 200ms measurement cost |
| Memory | 5s | Moderate change frequency |
| Disk | 30s | Changes very slowly (writes are buffered) |
| Network | Custom | Already has delta-based caching |

### Performance Improvement Calculation

**Without caching** (assuming 100 req/sec monitoring frequency):
- Blocking thread occupied: 200ms per request
- Concurrent requests: 100/sec × 0.2s = 20 threads permanently blocked
- Thread pool exhaustion risk: **HIGH**

**With 5-second caching**:
- Blocking thread occupied: 200ms per 5 seconds
- Cache hit rate: ~99.6% (499 cached / 500 total requests per 5s)
- Blocking operations: 1 per 5 seconds (vs 500 per 5 seconds)
- **96% reduction in blocking thread usage**

---

## DEFINITION OF DONE

- [ ] `CpuMetricsCache`, `MemoryMetricsCache`, `DiskMetricsCache` structs implemented
- [ ] All caches use `Arc<RwLock<Option<CachedReading>>>` pattern
- [ ] `MonitoringService` struct has cache fields
- [ ] Constructor initializes all caches with appropriate durations
- [ ] `get_current_cpu_usage()` checks cache before spawning
- [ ] `get_current_memory_usage()` checks cache before spawning
- [ ] `get_current_disk_usage()` checks cache before spawning
- [ ] Fresh measurements update their respective caches
- [ ] Trace logging for cache hits
- [ ] Debug logging for cache misses
- [ ] Cache management methods: `invalidate_cpu_cache()`, `cpu_cache_age()`, `is_cpu_cache_valid()`
- [ ] All methods have comprehensive documentation
- [ ] No compilation errors

---

## FILES TO MODIFY

**Single file** - all changes in one place:

1. [`packages/surrealdb/src/repository/monitoring_service.rs`](../packages/surrealdb/src/repository/monitoring_service.rs)
   - Add imports: `tokio::sync::RwLock`
   - Add cache structures: `CachedCpuReading`, `CpuMetricsCache`, etc.
   - Update `MonitoringService` struct (lines 22-28)
   - Update constructor (lines 30-37)
   - Update `get_current_cpu_usage()` (lines 322-334)
   - Update `get_current_memory_usage()` (lines 336-346)
   - Update `get_current_disk_usage()` (lines 348-391)
   - Add cache management methods

---

## RESEARCH CITATIONS

1. **sysinfo MINIMUM_CPU_UPDATE_INTERVAL**: [`tmp/sysinfo/src/unix/apple/system.rs`](../tmp/sysinfo/src/unix/apple/system.rs)
2. **sysinfo documentation**: [`tmp/sysinfo/README.md`](../tmp/sysinfo/README.md) lines 109-122
3. **sysinfo CPU interval explanation**: [`tmp/sysinfo/md_doc/minimum_cpu_update_interval.md`](../tmp/sysinfo/md_doc/minimum_cpu_update_interval.md)
4. **Existing network cache pattern**: [`packages/surrealdb/src/repository/monitoring_service.rs`](../packages/surrealdb/src/repository/monitoring_service.rs) lines 393-425
5. **moka cache usage example**: [`packages/surrealdb/src/repository/membership.rs`](../packages/surrealdb/src/repository/membership.rs) lines 1-50

---

## NOTES

- **5-second cache** balances accuracy vs performance (96% reduction in blocking calls)
- **RwLock** is optimal for high-read, low-write access patterns
- **Transparent to callers** - same API surface, no breaking changes
- **Cache invalidation available** for special cases requiring immediate fresh data
- **Consistent pattern** across CPU, memory, and disk metrics
- **Network already cached** with different pattern (delta-based)
