-- =====================================================
-- Migration: 107
-- Table: room_summaries
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/sync.rs
-- =====================================================

DEFINE TABLE room_summaries SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE room_summaries TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD joined_member_count ON TABLE room_summaries TYPE int DEFAULT 0;
DEFINE FIELD invited_member_count ON TABLE room_summaries TYPE int DEFAULT 0;
DEFINE FIELD heroes ON TABLE room_summaries TYPE array<string> DEFAULT [];
DEFINE FIELD name ON TABLE room_summaries TYPE option<string>;
DEFINE FIELD topic ON TABLE room_summaries TYPE option<string>;
DEFINE FIELD avatar ON TABLE room_summaries TYPE option<string>;
DEFINE FIELD last_event_ts ON TABLE room_summaries TYPE option<int>;

DEFINE INDEX rs_room_idx ON TABLE room_summaries COLUMNS room_id UNIQUE;
DEFINE INDEX rs_last_event_idx ON TABLE room_summaries COLUMNS last_event_ts;
