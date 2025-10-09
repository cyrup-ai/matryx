-- =====================================================
-- Migration: 099
-- Table: direct_to_device_messages
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/federation.rs
-- =====================================================

DEFINE TABLE direct_to_device_messages SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id = recipient_id
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD message_id ON TABLE direct_to_device_messages TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sender_id ON TABLE direct_to_device_messages TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD recipient_id ON TABLE direct_to_device_messages TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD device_id ON TABLE direct_to_device_messages TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD message_type ON TABLE direct_to_device_messages TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD content ON TABLE direct_to_device_messages TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE direct_to_device_messages TYPE datetime DEFAULT time::now();

DEFINE INDEX dtd_recipient_device_idx ON TABLE direct_to_device_messages COLUMNS recipient_id, device_id;
DEFINE INDEX dtd_created_idx ON TABLE direct_to_device_messages COLUMNS created_at;
