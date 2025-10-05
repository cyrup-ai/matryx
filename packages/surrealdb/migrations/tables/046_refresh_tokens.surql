-- =====================================================
-- Migration: 046
-- Table: refresh_tokens
-- Entity: packages/entity/src/types/refresh_tokens.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Refresh token table - For token refresh
DEFINE TABLE refresh_tokens SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD id ON TABLE refresh_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD token ON TABLE refresh_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE refresh_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE refresh_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE refresh_tokens TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE refresh_tokens TYPE datetime;
DEFINE FIELD used ON TABLE refresh_tokens TYPE bool DEFAULT false;
DEFINE FIELD revoked ON TABLE refresh_tokens TYPE bool DEFAULT false;
DEFINE FIELD is_revoked ON TABLE refresh_tokens TYPE bool DEFAULT false;
DEFINE FIELD revoked_at ON TABLE refresh_tokens TYPE option<datetime>;
DEFINE FIELD updated_at ON TABLE refresh_tokens TYPE datetime DEFAULT time::now();

DEFINE INDEX refresh_token_token_idx ON TABLE refresh_tokens COLUMNS token UNIQUE;
DEFINE INDEX refresh_token_user_idx ON TABLE refresh_tokens COLUMNS user_id;
DEFINE INDEX refresh_token_expires_idx ON TABLE refresh_tokens COLUMNS expires_at;
DEFINE INDEX refresh_token_revoked_idx ON TABLE refresh_tokens COLUMNS is_revoked;
