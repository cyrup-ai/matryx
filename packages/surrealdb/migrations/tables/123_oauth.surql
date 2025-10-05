-- =====================================================
-- Migration: 123
-- Table: oauth
-- Entity: OAuth session and token storage
-- Repositories: Generic OAuth storage
-- =====================================================

-- OAuth session and token storage for authentication flows
DEFINE TABLE oauth SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE oauth TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD provider ON TABLE oauth TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD access_token ON TABLE oauth TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD refresh_token ON TABLE oauth TYPE option<string>;

DEFINE FIELD expires_at ON TABLE oauth TYPE datetime;

DEFINE FIELD scope ON TABLE oauth TYPE array<string> DEFAULT [];

DEFINE FIELD created_at ON TABLE oauth TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns
DEFINE INDEX oauth_user_provider_idx ON TABLE oauth COLUMNS user_id, provider UNIQUE;
DEFINE INDEX oauth_access_token_idx ON TABLE oauth COLUMNS access_token UNIQUE;
DEFINE INDEX oauth_expires_idx ON TABLE oauth COLUMNS expires_at;
