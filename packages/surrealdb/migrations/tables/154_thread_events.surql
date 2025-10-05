-- =====================================================
-- Migration: 154
-- Table: thread_events
-- Entity: Thread event associations
-- Repositories: threads.rs
-- =====================================================

-- Thread event associations for message threading
DEFINE TABLE thread_events SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE thread_events TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD room_id ON TABLE thread_events TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '!') 
    AND string::contains($value, ':');

DEFINE FIELD thread_root_id ON TABLE thread_events TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '$') 
    AND string::contains($value, ':');

DEFINE FIELD event_id ON TABLE thread_events TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '$') 
    AND string::contains($value, ':');

DEFINE FIELD sender ON TABLE thread_events TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD created_at ON TABLE thread_events TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (threads.rs lines 204, 288, 401, 720)
DEFINE INDEX thread_events_id_idx ON TABLE thread_events COLUMNS id UNIQUE;
DEFINE INDEX thread_events_room_root_idx ON TABLE thread_events COLUMNS room_id, thread_root_id;
DEFINE INDEX thread_events_event_idx ON TABLE thread_events COLUMNS event_id;
