-- =====================================================
-- Migration: 156
-- Table: signing_keys
-- Entity: SigningKey (key_server.rs line 28)
-- Repositories: key_server.rs
-- =====================================================

-- Federation server signing key cache
DEFINE TABLE signing_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.server_name != NONE OR $auth.admin = true
        FOR create, update WHERE $auth.server_name != NONE OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD key_id ON TABLE signing_keys TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD server_name ON TABLE signing_keys TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD signing_key ON TABLE signing_keys TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD verify_key ON TABLE signing_keys TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD created_at ON TABLE signing_keys TYPE datetime DEFAULT time::now();

DEFINE FIELD expires_at ON TABLE signing_keys TYPE option<datetime>;

-- Indexes based on repository query patterns (key_server.rs lines 102, 154, 180)
DEFINE INDEX signing_keys_server_key_idx ON TABLE signing_keys COLUMNS server_name, key_id UNIQUE;
DEFINE INDEX signing_keys_expires_idx ON TABLE signing_keys COLUMNS expires_at;
