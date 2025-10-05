-- =====================================================
-- Migration: 037
-- Table: one_time_key_object
-- Entity: packages/entity/src/types/one_time_key_object.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- OneTimeKeyObject table - matches OneTimeKeyObject entity exactly
DEFINE TABLE one_time_key_object SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE one_time_key_object TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE one_time_key_object TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_id ON TABLE one_time_key_object TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key ON TABLE one_time_key_object TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD signatures ON TABLE one_time_key_object TYPE option<object>;
