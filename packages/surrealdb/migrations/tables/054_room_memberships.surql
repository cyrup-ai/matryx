-- =====================================================
-- Migration: 054
-- Table: room_memberships
-- Entity: packages/entity/src/types/room_memberships.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room memberships table - Alternative name for membership cache (queried by auth.rs, user.rs)
DEFINE TABLE room_memberships SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE room_memberships TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE room_memberships TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD membership ON TABLE room_memberships TYPE string ASSERT $value IN ['invite', 'join', 'leave', 'ban', 'knock'];
DEFINE FIELD display_name ON TABLE room_memberships TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE room_memberships TYPE option<string>;
DEFINE FIELD updated_at ON TABLE room_memberships TYPE datetime DEFAULT time::now();
