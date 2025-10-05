-- =====================================================
-- Migration: 016
-- Table: fallback_keys
-- Entity: packages/entity/src/types/fallback_keys.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Fallback keys table - E2E encryption fallback keys (queried by keys.rs)
DEFINE TABLE fallback_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE fallback_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE fallback_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_id ON TABLE fallback_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_data ON TABLE fallback_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm_type ON TABLE fallback_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD signatures ON TABLE fallback_keys TYPE option<object>;
DEFINE FIELD created_at ON TABLE fallback_keys TYPE datetime DEFAULT time::now();
DEFINE FIELD used ON TABLE fallback_keys TYPE bool DEFAULT false;
