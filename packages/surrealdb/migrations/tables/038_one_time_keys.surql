-- =====================================================
-- Migration: 038
-- Table: one_time_keys
-- Entity: packages/entity/src/types/one_time_keys.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- One-time keys table - E2E encryption one-time keys (queried by keys.rs)
DEFINE TABLE one_time_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE one_time_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE one_time_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_id ON TABLE one_time_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_data ON TABLE one_time_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm_type ON TABLE one_time_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD signatures ON TABLE one_time_keys TYPE option<object>;
DEFINE FIELD claimed ON TABLE one_time_keys TYPE bool DEFAULT false;
DEFINE FIELD claimed_at ON TABLE one_time_keys TYPE option<datetime>;
DEFINE FIELD claimed_by ON TABLE one_time_keys TYPE option<string>;
DEFINE FIELD created_at ON TABLE one_time_keys TYPE datetime DEFAULT time::now();

DEFINE INDEX one_time_keys_user_device_claimed_idx ON TABLE one_time_keys COLUMNS user_id, device_id, claimed;
DEFINE INDEX one_time_keys_user_device_key_idx ON TABLE one_time_keys COLUMNS user_id, device_id, key_id UNIQUE;
