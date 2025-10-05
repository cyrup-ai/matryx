-- =====================================================
-- Migration: 155
-- Table: thread_participation
-- Entity: Thread participation tracking
-- Repositories: threads.rs
-- =====================================================

-- Thread participant tracking for notifications
DEFINE TABLE thread_participation SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE thread_participation TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD room_id ON TABLE thread_participation TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '!') 
    AND string::contains($value, ':');

DEFINE FIELD thread_root_id ON TABLE thread_participation TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '$') 
    AND string::contains($value, ':');

DEFINE FIELD user_id ON TABLE thread_participation TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD participating ON TABLE thread_participation TYPE bool DEFAULT true;

DEFINE FIELD last_read_event_id ON TABLE thread_participation TYPE option<string>;

DEFINE FIELD created_at ON TABLE thread_participation TYPE datetime DEFAULT time::now();

DEFINE FIELD updated_at ON TABLE thread_participation TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (threads.rs lines 522, 597, 659)
DEFINE INDEX thread_participation_id_idx ON TABLE thread_participation COLUMNS id UNIQUE;
DEFINE INDEX thread_participation_room_root_user_idx ON TABLE thread_participation COLUMNS room_id, thread_root_id, user_id UNIQUE;
DEFINE INDEX thread_participation_user_idx ON TABLE thread_participation COLUMNS user_id;
