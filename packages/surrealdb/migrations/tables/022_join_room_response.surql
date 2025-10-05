-- =====================================================
-- Migration: 022
-- Table: join_room_response
-- Entity: packages/entity/src/types/join_room_response.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- JoinRoomResponse table - matches JoinRoomResponse entity exactly
DEFINE TABLE join_room_response SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD room_id ON TABLE join_room_response TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE join_room_response TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD timestamp ON TABLE join_room_response TYPE datetime DEFAULT time::now();
