-- =====================================================
-- Migration: 015
-- Table: event_threads
-- Entity: packages/entity/src/types/event_threads.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Thread relationships  
DEFINE TABLE event_threads SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.room_id AND membership = 'join')
        FOR create, update, delete WHERE $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.room_id AND membership = 'join');

DEFINE FIELD thread_root_id ON TABLE event_threads TYPE string ASSERT $value != "";
DEFINE FIELD event_id ON TABLE event_threads TYPE string ASSERT $value != "";
DEFINE FIELD room_id ON TABLE event_threads TYPE string ASSERT $value != "";
DEFINE FIELD thread_depth ON TABLE event_threads TYPE int DEFAULT 0;
DEFINE FIELD created_at ON TABLE event_threads TYPE datetime DEFAULT time::now();
