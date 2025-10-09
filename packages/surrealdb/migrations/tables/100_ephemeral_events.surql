-- =====================================================
-- Migration: 100
-- Table: ephemeral_events
-- Entity: packages/entity/src/types/ephemeral_event.rs
-- Repositories: packages/surrealdb/src/repository/sync.rs
-- =====================================================

DEFINE TABLE ephemeral_events SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD event_type ON TABLE ephemeral_events TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD content ON TABLE ephemeral_events TYPE object DEFAULT {};
DEFINE FIELD room_id ON TABLE ephemeral_events TYPE option<string> ASSERT ($value IS NONE) OR (string::starts_with($value, '!') AND string::contains($value, ':'));
DEFINE FIELD sender ON TABLE ephemeral_events TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD timestamp ON TABLE ephemeral_events TYPE datetime DEFAULT time::now();

DEFINE INDEX ephemeral_room_type_idx ON TABLE ephemeral_events COLUMNS room_id, event_type;
DEFINE INDEX ephemeral_room_timestamp_idx ON TABLE ephemeral_events COLUMNS room_id, timestamp;
DEFINE INDEX ephemeral_timestamp_idx ON TABLE ephemeral_events COLUMNS timestamp;
