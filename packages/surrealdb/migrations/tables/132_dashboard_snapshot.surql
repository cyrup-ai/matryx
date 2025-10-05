-- =====================================================
-- Migration: 132
-- Table: dashboard_snapshot
-- Entity: DashboardSnapshot (monitoring.rs line 386)
-- Repositories: monitoring.rs
-- =====================================================

-- Dashboard data snapshots for monitoring UI
DEFINE TABLE dashboard_snapshot SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE $auth.admin = true OR $auth.monitoring = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE dashboard_snapshot TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD dashboard_id ON TABLE dashboard_snapshot TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD data ON TABLE dashboard_snapshot TYPE object DEFAULT {};

DEFINE FIELD timestamp ON TABLE dashboard_snapshot TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (monitoring.rs line 234)
DEFINE INDEX dashboard_snapshot_id_idx ON TABLE dashboard_snapshot COLUMNS id UNIQUE;
DEFINE INDEX dashboard_snapshot_dashboard_timestamp_idx ON TABLE dashboard_snapshot COLUMNS dashboard_id, timestamp;
