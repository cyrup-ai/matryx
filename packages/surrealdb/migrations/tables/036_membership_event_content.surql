-- =====================================================
-- Migration: 036
-- Table: membership_event_content
-- Entity: packages/entity/src/types/membership_event_content.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

DEFINE TABLE membership_event_content SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE  -- Authenticated users can read membership they have access to
        FOR create, update WHERE $auth.user_id != NONE  -- Only authenticated users can modify membership
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD event_id ON TABLE membership_event_content TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD sender ON TABLE membership_event_content TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD origin_server_ts ON TABLE membership_event_content TYPE int ASSERT $value > 0;
DEFINE FIELD membership ON TABLE membership_event_content TYPE string ASSERT $value IN ['invite', 'join', 'leave', 'ban', 'knock'];
DEFINE FIELD user_id ON TABLE membership_event_content TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD room_id ON TABLE membership_event_content TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD display_name ON TABLE membership_event_content TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE membership_event_content TYPE option<string>;
DEFINE FIELD reason ON TABLE membership_event_content TYPE option<string>;
DEFINE FIELD third_party_invite ON TABLE membership_event_content TYPE option<object>;
