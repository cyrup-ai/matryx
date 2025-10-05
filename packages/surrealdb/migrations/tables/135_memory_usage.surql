-- =====================================================
-- Migration: 135
-- Table: memory_usage
-- Entity: MemoryUsage (performance.rs line 369)
-- Repositories: performance.rs
-- =====================================================

-- Memory usage tracking per component for monitoring
DEFINE TABLE memory_usage SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE $auth.admin = true OR $auth.monitoring = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE memory_usage TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD component ON TABLE memory_usage TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD memory_mb ON TABLE memory_usage TYPE float;

DEFINE FIELD timestamp ON TABLE memory_usage TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (performance.rs line 183)
DEFINE INDEX memory_usage_id_idx ON TABLE memory_usage COLUMNS id UNIQUE;
DEFINE INDEX memory_usage_component_timestamp_idx ON TABLE memory_usage COLUMNS component, timestamp;
DEFINE INDEX memory_usage_timestamp_idx ON TABLE memory_usage COLUMNS timestamp;
