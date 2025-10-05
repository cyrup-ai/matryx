-- =====================================================
-- Migration: 079
-- Table: user_access_tokens
-- Entity: packages/entity/src/types/user_access_tokens.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- User access tokens table for Matrix client authentication
DEFINE TABLE user_access_tokens SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD token ON TABLE user_access_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE user_access_tokens TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD device_id ON TABLE user_access_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE user_access_tokens TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE user_access_tokens TYPE option<datetime>;
DEFINE FIELD last_used_at ON TABLE user_access_tokens TYPE option<datetime>;
DEFINE FIELD is_active ON TABLE user_access_tokens TYPE bool DEFAULT true;
