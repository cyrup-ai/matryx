-- =====================================================
-- Migration: 134
-- Table: request_timing
-- Entity: RequestTiming (performance.rs line 355)
-- Repositories: performance.rs
-- =====================================================

-- HTTP request timing data for performance monitoring
DEFINE TABLE request_timing SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE $auth.admin = true OR $auth.monitoring = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE request_timing TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD endpoint ON TABLE request_timing TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD method ON TABLE request_timing TYPE string 
    ASSERT $value IN ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'OPTIONS', 'HEAD'];

DEFINE FIELD duration_ms ON TABLE request_timing TYPE float;

DEFINE FIELD status_code ON TABLE request_timing TYPE int;

DEFINE FIELD timestamp ON TABLE request_timing TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (performance.rs lines 130-238)
DEFINE INDEX request_timing_id_idx ON TABLE request_timing COLUMNS id UNIQUE;
DEFINE INDEX request_timing_timestamp_idx ON TABLE request_timing COLUMNS timestamp;
DEFINE INDEX request_timing_endpoint_timestamp_idx ON TABLE request_timing COLUMNS endpoint, timestamp;
DEFINE INDEX request_timing_duration_idx ON TABLE request_timing COLUMNS duration_ms;
