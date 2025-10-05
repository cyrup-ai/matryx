-- =====================================================
-- Migration: 005
-- Table: cross_signing_key
-- Entity: packages/entity/src/types/cross_signing_key.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- CrossSigningKey table - matches CrossSigningKey entity exactly
DEFINE TABLE cross_signing_key SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Cross-signing keys are public for verification in Matrix
        FOR create, update, delete WHERE $auth.user_id != NONE;  -- Users manage their own cross-signing keys

DEFINE FIELD user_id ON TABLE cross_signing_key TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD usage ON TABLE cross_signing_key TYPE array<string> ASSERT array::len($value) > 0;
DEFINE FIELD keys ON TABLE cross_signing_key TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD signatures ON TABLE cross_signing_key TYPE option<object>;
