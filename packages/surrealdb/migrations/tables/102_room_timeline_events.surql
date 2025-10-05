-- =====================================================
-- Migration: 102
-- Table: room_timeline_events
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/thread.rs, packages/surrealdb/src/repository/sync.rs
-- =====================================================

DEFINE TABLE room_timeline_events SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD event_id ON TABLE room_timeline_events TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE room_timeline_events TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sender ON TABLE room_timeline_events TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD event_type ON TABLE room_timeline_events TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD content ON TABLE room_timeline_events TYPE object DEFAULT {};
DEFINE FIELD origin_server_ts ON TABLE room_timeline_events TYPE int;
DEFINE FIELD unsigned ON TABLE room_timeline_events TYPE option<object>;

DEFINE INDEX rte_room_ts_idx ON TABLE room_timeline_events COLUMNS room_id, origin_server_ts;
DEFINE INDEX rte_event_id_idx ON TABLE room_timeline_events COLUMNS event_id UNIQUE;
DEFINE INDEX rte_sender_idx ON TABLE room_timeline_events COLUMNS sender;
