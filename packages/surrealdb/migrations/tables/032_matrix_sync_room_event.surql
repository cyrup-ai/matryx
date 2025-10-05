-- =====================================================
-- Migration: 032
-- Table: matrix_sync_room_event
-- Entity: packages/entity/src/types/matrix_sync_room_event.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Matrix sync room events for /sync rooms section
DEFINE TABLE matrix_sync_room_event SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD user_id ON TABLE matrix_sync_room_event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD room_id ON TABLE matrix_sync_room_event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD event_id ON TABLE matrix_sync_room_event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD event_type ON TABLE matrix_sync_room_event TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_room_event TYPE int ASSERT $value > 0;
DEFINE FIELD timeline_event ON TABLE matrix_sync_room_event TYPE object DEFAULT {};
DEFINE FIELD state_event ON TABLE matrix_sync_room_event TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE matrix_sync_room_event TYPE datetime DEFAULT time::now();
