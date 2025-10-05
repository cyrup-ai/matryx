-- =====================================================
-- Migration: 142
-- Table: bridge_metrics
-- Entity: Bridge performance metrics
-- Repositories: bridge.rs
-- =====================================================

-- Bridge performance metrics for monitoring
DEFINE TABLE bridge_metrics SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.monitoring = true
        FOR create, update WHERE $auth.admin = true OR $auth.monitoring = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD bridge_id ON TABLE bridge_metrics TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD messages_24h ON TABLE bridge_metrics TYPE int DEFAULT 0;

DEFINE FIELD errors_24h ON TABLE bridge_metrics TYPE int DEFAULT 0;

DEFINE FIELD avg_response_time ON TABLE bridge_metrics TYPE int DEFAULT 0;

DEFINE FIELD uptime_percentage ON TABLE bridge_metrics TYPE float DEFAULT 100.0;

DEFINE FIELD last_health_check ON TABLE bridge_metrics TYPE option<datetime>;

DEFINE FIELD last_updated ON TABLE bridge_metrics TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (bridge.rs line 327)
DEFINE INDEX bridge_metrics_bridge_id_idx ON TABLE bridge_metrics COLUMNS bridge_id UNIQUE;
