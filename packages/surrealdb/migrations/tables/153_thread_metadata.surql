-- =====================================================
-- Migration: 153
-- Table: thread_metadata
-- Entity: Thread metadata for Matrix threads
-- Repositories: threads.rs
-- =====================================================

-- Thread metadata for message threading
DEFINE TABLE thread_metadata SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE thread_metadata TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD room_id ON TABLE thread_metadata TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '!') 
    AND string::contains($value, ':');

DEFINE FIELD thread_root_id ON TABLE thread_metadata TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '$') 
    AND string::contains($value, ':');

DEFINE FIELD participant_count ON TABLE thread_metadata TYPE int DEFAULT 0;

DEFINE FIELD reply_count ON TABLE thread_metadata TYPE int DEFAULT 0;

DEFINE FIELD last_event_id ON TABLE thread_metadata TYPE option<string>;

DEFINE FIELD last_event_timestamp ON TABLE thread_metadata TYPE option<datetime>;

DEFINE FIELD created_at ON TABLE thread_metadata TYPE datetime DEFAULT time::now();

DEFINE FIELD updated_at ON TABLE thread_metadata TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (threads.rs lines 108, 272, 346)
DEFINE INDEX thread_metadata_id_idx ON TABLE thread_metadata COLUMNS id UNIQUE;
DEFINE INDEX thread_metadata_room_root_idx ON TABLE thread_metadata COLUMNS room_id, thread_root_id UNIQUE;
