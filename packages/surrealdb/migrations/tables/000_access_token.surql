-- =====================================================
-- Migration: 000
-- Table: access_token
-- Entity: packages/entity/src/types/access_token.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Access token table - For token-based authentication
DEFINE TABLE access_token SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD token ON TABLE access_token TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE access_token TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE access_token TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE access_token TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE access_token TYPE option<datetime>;
DEFINE FIELD last_used_at ON TABLE access_token TYPE option<datetime>;

DEFINE INDEX access_token_token_idx ON TABLE access_token COLUMNS token UNIQUE;
DEFINE INDEX access_token_user_device_idx ON TABLE access_token COLUMNS user_id, device_id;
DEFINE INDEX access_token_expires_idx ON TABLE access_token COLUMNS expires_at;
