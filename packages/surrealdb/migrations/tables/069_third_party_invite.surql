-- =====================================================
-- Migration: 069
-- Table: third_party_invite
-- Entity: packages/entity/src/types/third_party_invite.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

DEFINE TABLE third_party_invite SCHEMAFULL
    PERMISSIONS
        FOR select WHERE room_id IN (SELECT room_id FROM membership_event_content WHERE user_id = $auth.user_id)
        FOR create, update, delete WHERE sender = $auth.user_id OR $auth.admin = true;

DEFINE FIELD address ON TABLE third_party_invite TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD medium ON TABLE third_party_invite TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD mxid ON TABLE third_party_invite TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE third_party_invite TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sender ON TABLE third_party_invite TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD signed ON TABLE third_party_invite TYPE object ASSERT object::len($value) > 0;
