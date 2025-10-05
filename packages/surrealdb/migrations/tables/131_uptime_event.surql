-- =====================================================
-- Migration: 131
-- Table: uptime_event
-- Entity: UptimeEvent (monitoring.rs line 379)
-- Repositories: monitoring.rs
-- =====================================================

-- Server uptime event tracking (start, stop, restart, maintenance)
DEFINE TABLE uptime_event SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE uptime_event TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD event_type ON TABLE uptime_event TYPE string 
    ASSERT $value IN ['Start', 'Stop', 'Restart', 'Maintenance'];

DEFINE FIELD timestamp ON TABLE uptime_event TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (monitoring.rs line 120)
DEFINE INDEX uptime_event_id_idx ON TABLE uptime_event COLUMNS id UNIQUE;
DEFINE INDEX uptime_event_type_timestamp_idx ON TABLE uptime_event COLUMNS event_type, timestamp;
DEFINE INDEX uptime_event_timestamp_idx ON TABLE uptime_event COLUMNS timestamp;
