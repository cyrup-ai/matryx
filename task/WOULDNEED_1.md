# WOULDNEED_1: Fix Misleading Comment in Bridge Statistics

## STATUS

**RATING: 8/10** - Implementation is excellent, one minor comment cleanup needed.

## COMPLETED ITEMS ✅

- **Problem 1 (Lazy Loading Metrics)**: FIXED - Atomic counters now loaded correctly
  - `total_members_filtered: self.members_filtered_out.load(Ordering::Relaxed)` ✅
  - `db_queries_avoided: self.db_queries_avoided.load(Ordering::Relaxed)` ✅
  
- **Problem 2 (Third-Party Protocol Messages)**: FIXED - Messages aggregated from bridge_metrics ✅
  - Correctly aggregates `messages_24h` from all protocol bridges
  - Uses `get_bridge_performance_metrics()` as recommended
  
- **Problem 3 (Bridge Statistics)**: MOSTLY FIXED - Real data from bridge_metrics ✅
  - `messages_bridged_24h: perf_metrics.messages_24h` ✅
  - `uptime_percentage: perf_metrics.uptime_percentage` ✅

## REMAINING ISSUE ⚠️

**File:** `packages/surrealdb/src/repository/bridge.rs`  
**Line:** 129  
**Current Code:**
```rust
last_error: None, // Would need error tracking
```

**Problem:** The comment is misleading. It suggests the feature isn't implemented when the reality is:
- The database schema has `bridge_metrics.errors_24h` (error count only)
- No error message text storage exists in the schema
- Returning `None` is correct, but the comment should explain why

**Fix Options:**

**Option 1 (Recommended):** Update comment to be accurate
```rust
last_error: None, // No error message storage in schema (only error counts tracked)
```

**Option 2 (Cleaner):** Remove the comment entirely
```rust
last_error: None,
```

## DEFINITION OF DONE

- [ ] Update or remove the misleading comment at bridge.rs:129
- [ ] Comment accurately reflects the schema limitation (if kept)

## NOTES

- Pre-existing compilation error in `presence_streams.rs` is unrelated to this task
- All metric tracking implementations are correct and production-ready
- No further code changes needed beyond the comment

---

**Last Updated:** 2025-10-08 (Code Review)  
**Estimated Fix Time:** 1 minute
