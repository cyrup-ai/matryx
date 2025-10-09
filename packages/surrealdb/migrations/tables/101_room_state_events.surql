-- =====================================================
-- Migration: 101
-- Table: room_state_events
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/pusher.rs, packages/surrealdb/src/repository/room.rs
-- =====================================================

DEFINE TABLE room_state_events SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD event_id ON TABLE room_state_events TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD room_id ON TABLE room_state_events TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD sender ON TABLE room_state_events TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD event_type ON TABLE room_state_events TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD state_key ON TABLE room_state_events TYPE string;
DEFINE FIELD content ON TABLE room_state_events TYPE object DEFAULT {};
DEFINE FIELD origin_server_ts ON TABLE room_state_events TYPE int;

DEFINE INDEX rse_room_idx ON TABLE room_state_events COLUMNS room_id;
DEFINE INDEX rse_room_type_idx ON TABLE room_state_events COLUMNS room_id, event_type;
DEFINE INDEX rse_event_id_idx ON TABLE room_state_events COLUMNS event_id UNIQUE;
