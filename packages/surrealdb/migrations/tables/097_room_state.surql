-- =====================================================
-- Migration: 097
-- Table: room_state
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/sync.rs
-- =====================================================

DEFINE TABLE room_state SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE room_state TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD event_id ON TABLE room_state TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD event_type ON TABLE room_state TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD state_key ON TABLE room_state TYPE string;
DEFINE FIELD content ON TABLE room_state TYPE object DEFAULT {};
DEFINE FIELD sender ON TABLE room_state TYPE string;
DEFINE FIELD origin_server_ts ON TABLE room_state TYPE int;

DEFINE INDEX room_state_room_idx ON TABLE room_state COLUMNS room_id;
DEFINE INDEX room_state_room_ts_idx ON TABLE room_state COLUMNS room_id, origin_server_ts;
DEFINE INDEX room_state_composite_idx ON TABLE room_state COLUMNS room_id, event_type, state_key UNIQUE;
