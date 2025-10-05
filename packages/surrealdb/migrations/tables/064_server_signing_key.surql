-- =====================================================
-- Migration: 064
-- Table: server_signing_key
-- Entity: packages/entity/src/types/server_signing_key.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Server signing key table - Individual server signing key (queried by session.rs)
DEFINE TABLE server_signing_key SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true -- Server keys are public for verification
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD key_id ON TABLE server_signing_key TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_data ON TABLE server_signing_key TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm ON TABLE server_signing_key TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE server_signing_key TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE server_signing_key TYPE option<datetime>;
DEFINE FIELD is_active ON TABLE server_signing_key TYPE bool DEFAULT true;
