-- =====================================================
-- Migration: 130
-- Table: metric
-- Entity: Metric (metrics.rs line 263)
-- Repositories: metrics.rs
-- =====================================================

-- Metrics storage for Prometheus-compatible monitoring
DEFINE TABLE metric SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE $auth.admin = true OR $auth.monitoring = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD name ON TABLE metric TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD value ON TABLE metric TYPE float;

DEFINE FIELD labels ON TABLE metric TYPE object DEFAULT {};

DEFINE FIELD timestamp ON TABLE metric TYPE datetime DEFAULT time::now();

DEFINE FIELD metric_type ON TABLE metric TYPE string 
    ASSERT $value IN ['counter', 'gauge', 'histogram', 'summary'];

-- Indexes based on repository query patterns (metrics.rs)
DEFINE INDEX metric_name_timestamp_idx ON TABLE metric COLUMNS name, timestamp;
DEFINE INDEX metric_timestamp_idx ON TABLE metric COLUMNS timestamp;
