-- =====================================================
-- Migration: 129
-- Table: health_check
-- Entity: HealthCheck (monitoring.rs line 372)
-- Repositories: monitoring.rs
-- =====================================================

-- System health check records for monitoring
DEFINE TABLE health_check SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE $auth.admin = true OR $auth.monitoring = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE health_check TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD component ON TABLE health_check TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD status ON TABLE health_check TYPE string 
    ASSERT $value IN ['Healthy', 'Degraded', 'Unhealthy'];

DEFINE FIELD details ON TABLE health_check TYPE option<string>;

DEFINE FIELD timestamp ON TABLE health_check TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (monitoring.rs lines 92-95)
DEFINE INDEX health_check_id_idx ON TABLE health_check COLUMNS id UNIQUE;
DEFINE INDEX health_check_component_timestamp_idx ON TABLE health_check COLUMNS component, timestamp;
DEFINE INDEX health_check_timestamp_idx ON TABLE health_check COLUMNS timestamp;
