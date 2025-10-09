-- =====================================================
-- Migration: 117
-- Table: room_capabilities
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/capabilities.rs
-- =====================================================

DEFINE TABLE room_capabilities SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD version ON TABLE room_capabilities TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD features ON TABLE room_capabilities TYPE array<string> DEFAULT [];
DEFINE FIELD state_resolution ON TABLE room_capabilities TYPE string 
    ASSERT string::is::not::empty($value);
DEFINE FIELD supported_event_types ON TABLE room_capabilities TYPE array<string> DEFAULT [];

DEFINE INDEX rc_version_idx ON TABLE room_capabilities COLUMNS version UNIQUE;
