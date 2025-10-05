-- =====================================================
-- Migration: 006
-- Table: cross_signing_keys
-- Entity: packages/entity/src/types/cross_signing_keys.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Cross-signing keys table - E2E encryption cross-signing keys (queried by keys.rs)
DEFINE TABLE cross_signing_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Cross-signing keys are public for verification
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE cross_signing_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_type ON TABLE cross_signing_keys TYPE string ASSERT $value IN ['master', 'self_signing', 'user_signing'];
DEFINE FIELD key_id ON TABLE cross_signing_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_data ON TABLE cross_signing_keys TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD usage ON TABLE cross_signing_keys TYPE array<string> DEFAULT [];
DEFINE FIELD signatures ON TABLE cross_signing_keys TYPE option<object>;
DEFINE FIELD created_at ON TABLE cross_signing_keys TYPE datetime DEFAULT time::now();
