# CPUCACHE_1: Complete Cache Management API Consistency

**Status**: 🟡 **INCOMPLETE** - Missing management methods for memory and disk caches  
**Priority**: LOW (API Consistency Enhancement)  
**Package**: packages/surrealdb  
**File**: `packages/surrealdb/src/repository/monitoring_service.rs`

---

## QA REVIEW RATING: 8/10

### COMPLETED ITEMS (Production Quality)
✅ All three cache miss debug logging statements implemented (lines 609, 638, 661)
✅ Logging messages perfectly consistent across all cache types
✅ Code compiles without errors (only 2 unrelated warnings in sync.rs)
✅ Cache structures properly implemented with RwLock pattern
✅ Cache durations correctly optimized (CPU: 5s, Memory: 5s, Disk: 30s)
✅ Excellent documentation and inline comments
✅ CPU cache management methods complete (invalidate, age check, validity check)

### OUTSTANDING ISSUE: Inconsistent Cache Management API

**Problem**: Only CPU cache has management methods. Memory and disk caches lack equivalent APIs for consistency.

**Current State**:
- CPU cache has: `invalidate_cpu_cache()`, `cpu_cache_age()`, `is_cpu_cache_valid()`
- Memory cache has: ❌ No management methods
- Disk cache has: ❌ No management methods

---

## REQUIRED IMPLEMENTATION

Add the following management methods to achieve true consistency across all three cache types:

### 1. Memory Cache Management Methods

Add after `is_cpu_cache_valid()` method (around line 577):

```rust
/// Invalidate the memory metrics cache
///
/// Forces the next call to `get_current_memory_usage()` to fetch a fresh
/// measurement regardless of cache age.
pub async fn invalidate_memory_cache(&self) {
    let mut cache = self.memory_cache.last_reading.write().await;
    *cache = None;
    tracing::debug!("Memory cache manually invalidated");
}

/// Get the age of the current cached memory value
///
/// Returns `None` if no cached value exists, otherwise returns the
/// duration since the value was cached.
pub async fn memory_cache_age(&self) -> Option<std::time::Duration> {
    let cache = self.memory_cache.last_reading.read().await;
    cache.as_ref().map(|reading| reading.timestamp.elapsed())
}

/// Check if memory cache is currently valid (not expired)
///
/// Returns `true` if a cached value exists and is within the cache duration.
pub async fn is_memory_cache_valid(&self) -> bool {
    self.memory_cache.get().await.is_some()
}
```

### 2. Disk Cache Management Methods

Add after memory cache management methods:

```rust
/// Invalidate the disk metrics cache
///
/// Forces the next call to `get_current_disk_usage()` to fetch a fresh
/// measurement regardless of cache age.
pub async fn invalidate_disk_cache(&self) {
    let mut cache = self.disk_cache.last_reading.write().await;
    *cache = None;
    tracing::debug!("Disk cache manually invalidated");
}

/// Get the age of the current cached disk value
///
/// Returns `None` if no cached value exists, otherwise returns the
/// duration since the value was cached.
pub async fn disk_cache_age(&self) -> Option<std::time::Duration> {
    let cache = self.disk_cache.last_reading.read().await;
    cache.as_ref().map(|reading| reading.timestamp.elapsed())
}

/// Check if disk cache is currently valid (not expired)
///
/// Returns `true` if a cached value exists and is within the cache duration.
pub async fn is_disk_cache_valid(&self) -> bool {
    self.disk_cache.get().await.is_some()
}
```

---

## RATIONALE FOR CONSISTENCY

Having management methods for all three cache types provides:

1. **Uniform API**: Developers can invalidate/check any cache type using the same pattern
2. **Testing Support**: Integration tests can control all cache states consistently  
3. **Debugging**: Diagnostics can check age/validity of all caches uniformly
4. **Future-Proofing**: If new cache types are added, the pattern is clear

**Example Use Case**:
```rust
// Currently possible:
service.invalidate_cpu_cache().await;

// Should also be possible:
service.invalidate_memory_cache().await;
service.invalidate_disk_cache().await;

// For diagnostics:
println!("CPU cache age: {:?}", service.cpu_cache_age().await);
println!("Memory cache age: {:?}", service.memory_cache_age().await);
println!("Disk cache age: {:?}", service.disk_cache_age().await);
```

---

## DEFINITION OF DONE

- [x] CPU cache miss logging implemented
- [x] Memory cache miss logging implemented
- [x] Disk cache miss logging implemented
- [x] CPU cache management methods implemented
- [ ] **Memory cache management methods implemented** ← OUTSTANDING
- [ ] **Disk cache management methods implemented** ← OUTSTANDING
- [x] Code compiles without errors
- [x] Cache durations optimized
- [x] Comprehensive documentation

---

## VERIFICATION COMMANDS

After implementing the missing methods:

```bash
# Verify all management methods exist
grep -n "pub async fn.*invalidate.*cache" packages/surrealdb/src/repository/monitoring_service.rs
grep -n "pub async fn.*cache_age" packages/surrealdb/src/repository/monitoring_service.rs
grep -n "pub async fn.*is_.*cache_valid" packages/surrealdb/src/repository/monitoring_service.rs

# Expected: 3 invalidate methods, 3 age methods, 3 validity methods

# Compile check
cargo check -p matryx_surrealdb

# Full build
cargo build -p matryx_surrealdb
```

---

**Task Status**: 🟡 INCOMPLETE - Add management methods for memory and disk caches to match CPU cache API
