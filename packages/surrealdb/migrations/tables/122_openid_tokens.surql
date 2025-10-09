-- =====================================================
-- Migration: 122
-- Table: openid_tokens
-- Entity: OpenID Connect access tokens
-- Repositories: auth.rs
-- =====================================================

-- OpenID Connect access token storage
DEFINE TABLE openid_tokens SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.user_id != NONE
        FOR delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD access_token ON TABLE openid_tokens TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD user_id ON TABLE openid_tokens TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD expires_at ON TABLE openid_tokens TYPE datetime 
    ASSERT $value > time::now();

DEFINE FIELD created_at ON TABLE openid_tokens TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns
DEFINE INDEX openid_tokens_access_token_idx ON TABLE openid_tokens COLUMNS access_token UNIQUE;
DEFINE INDEX openid_tokens_user_idx ON TABLE openid_tokens COLUMNS user_id;
DEFINE INDEX openid_tokens_expires_idx ON TABLE openid_tokens COLUMNS expires_at;
