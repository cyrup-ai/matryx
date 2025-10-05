-- =====================================================
-- Migration: 035
-- Table: membership
-- Entity: packages/entity/src/types/membership.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Membership table - Room membership tracking (matches Membership entity)
DEFINE TABLE membership SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE membership TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD user_id ON TABLE membership TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD membership ON TABLE membership TYPE string ASSERT $value IN ['invite', 'join', 'leave', 'ban', 'knock'];
DEFINE FIELD display_name ON TABLE membership TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE membership TYPE option<string>;
DEFINE FIELD reason ON TABLE membership TYPE option<string>;
DEFINE FIELD invited_by ON TABLE membership TYPE option<string>;
DEFINE FIELD updated_at ON TABLE membership TYPE datetime DEFAULT time::now();

DEFINE INDEX membership_room_user_idx ON TABLE membership COLUMNS room_id, user_id UNIQUE;
DEFINE INDEX membership_room_idx ON TABLE membership COLUMNS room_id;
DEFINE INDEX membership_user_idx ON TABLE membership COLUMNS user_id;
DEFINE INDEX membership_room_state_idx ON TABLE membership COLUMNS room_id, membership;
DEFINE INDEX membership_user_state_idx ON TABLE membership COLUMNS user_id, membership;
DEFINE INDEX membership_state_idx ON TABLE membership COLUMNS membership;
