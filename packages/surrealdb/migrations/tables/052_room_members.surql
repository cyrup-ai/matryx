-- =====================================================
-- Migration: 052
-- Table: room_members
-- Entity: packages/entity/src/types/room_members.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room members table - Alternative membership tracking (queried by multiple repositories)
DEFINE TABLE room_members SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE room_members TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE room_members TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD membership ON TABLE room_members TYPE string ASSERT $value IN ['invite', 'join', 'leave', 'ban', 'knock'];
DEFINE FIELD display_name ON TABLE room_members TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE room_members TYPE option<string>;
DEFINE FIELD reason ON TABLE room_members TYPE option<string>;
DEFINE FIELD invited_by ON TABLE room_members TYPE option<string>;
DEFINE FIELD joined_at ON TABLE room_members TYPE option<datetime>;
DEFINE FIELD updated_at ON TABLE room_members TYPE datetime DEFAULT time::now();
