-- =====================================================
-- Migration: 110
-- Table: lazy_loading
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/performance.rs
-- =====================================================

DEFINE TABLE lazy_loading SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE lazy_loading TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE lazy_loading TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD load_time_ms ON TABLE lazy_loading TYPE float;
DEFINE FIELD memory_saved_mb ON TABLE lazy_loading TYPE float;
DEFINE FIELD timestamp ON TABLE lazy_loading TYPE datetime DEFAULT time::now();

DEFINE INDEX ll_timestamp_idx ON TABLE lazy_loading COLUMNS timestamp;
DEFINE INDEX ll_room_user_idx ON TABLE lazy_loading COLUMNS room_id, user_id;
