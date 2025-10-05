-- =====================================================
-- Migration: 065
-- Table: server_signing_keys
-- Entity: packages/entity/src/types/server_signing_keys.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Server signing keys table for Ed25519 federation signing and verification
DEFINE TABLE server_signing_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true -- Server keys are public for verification
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD key_id ON TABLE server_signing_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD server_name ON TABLE server_signing_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD private_key ON TABLE server_signing_keys TYPE option<string>; -- Only present for our own keys
DEFINE FIELD public_key ON TABLE server_signing_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE server_signing_keys TYPE datetime DEFAULT time::now();
DEFINE FIELD fetched_at ON TABLE server_signing_keys TYPE option<datetime>; -- For remote keys
DEFINE FIELD expires_at ON TABLE server_signing_keys TYPE option<datetime>;
DEFINE FIELD is_active ON TABLE server_signing_keys TYPE bool DEFAULT true;
DEFINE FIELD valid_until_ts ON TABLE server_signing_keys TYPE option<int>; -- Matrix timestamp
DEFINE FIELD signatures ON TABLE server_signing_keys TYPE option<object>; -- For remote key verification
