-- =====================================================
-- Migration: 133
-- Table: alert
-- Entity: Alert (metrics.rs line 361)
-- Repositories: monitoring.rs
-- =====================================================

-- Alert system for critical events and warnings
DEFINE TABLE alert SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE $auth.admin = true OR $auth.monitoring = true
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE alert TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD name ON TABLE alert TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD severity ON TABLE alert TYPE string 
    ASSERT $value IN ['Critical', 'Warning', 'Info'];

DEFINE FIELD message ON TABLE alert TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD created_at ON TABLE alert TYPE datetime DEFAULT time::now();

DEFINE FIELD resolved_at ON TABLE alert TYPE option<datetime>;

DEFINE FIELD resolved_by ON TABLE alert TYPE option<string>;

DEFINE FIELD labels ON TABLE alert TYPE object DEFAULT {};

-- Indexes based on repository query patterns (monitoring.rs)
DEFINE INDEX alert_id_idx ON TABLE alert COLUMNS id UNIQUE;
DEFINE INDEX alert_resolved_created_idx ON TABLE alert COLUMNS resolved_at, created_at;
DEFINE INDEX alert_severity_created_idx ON TABLE alert COLUMNS severity, created_at;
