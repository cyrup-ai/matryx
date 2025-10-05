-- =====================================================
-- Migration: 136
-- Table: cache_stats
-- Entity: CacheStats (performance.rs line 377)
-- Repositories: performance.rs
-- =====================================================

-- Cache hit/miss statistics for performance monitoring
DEFINE TABLE cache_stats SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE $auth.admin = true OR $auth.monitoring = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE cache_stats TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD cache_name ON TABLE cache_stats TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD hits ON TABLE cache_stats TYPE int DEFAULT 0;

DEFINE FIELD misses ON TABLE cache_stats TYPE int DEFAULT 0;

DEFINE FIELD hit_rate ON TABLE cache_stats TYPE float DEFAULT 0.0;

DEFINE FIELD timestamp ON TABLE cache_stats TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (performance.rs line 319)
DEFINE INDEX cache_stats_id_idx ON TABLE cache_stats COLUMNS id UNIQUE;
DEFINE INDEX cache_stats_cache_timestamp_idx ON TABLE cache_stats COLUMNS cache_name, timestamp;
DEFINE INDEX cache_stats_timestamp_idx ON TABLE cache_stats COLUMNS timestamp;
