-- =====================================================
-- Migration: 053
-- Table: room_membership
-- Entity: packages/entity/src/types/room_membership.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Matrix room membership cache for efficient LiveQuery lookups
DEFINE TABLE room_membership SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD room_id ON TABLE room_membership TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE room_membership TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD membership ON TABLE room_membership TYPE string ASSERT $value IN ['invite', 'join', 'leave', 'ban', 'knock'];
DEFINE FIELD display_name ON TABLE room_membership TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE room_membership TYPE option<string>;
