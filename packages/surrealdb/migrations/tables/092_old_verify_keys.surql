-- =====================================================
-- Migration: 092
-- Table: old_verify_keys
-- Entity: packages/entity/src/types/old_verify_key.rs
-- Repositories: key_server.rs
-- =====================================================

-- Store expired server signing keys for federation
DEFINE TABLE old_verify_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update WHERE $auth.server_name != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD server_name ON TABLE old_verify_keys TYPE string 
    ASSERT string::is::not::empty($value) 
    AND (string::contains($value, ':') OR string::contains($value, '.'));

DEFINE FIELD key_id ON TABLE old_verify_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key ON TABLE old_verify_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD expired_ts ON TABLE old_verify_keys TYPE int ASSERT $value > 0;
DEFINE FIELD created_at ON TABLE old_verify_keys TYPE datetime DEFAULT time::now();

DEFINE INDEX old_verify_keys_server_key_idx ON TABLE old_verify_keys 
    COLUMNS server_name, key_id UNIQUE;
DEFINE INDEX old_verify_keys_expired_idx ON TABLE old_verify_keys 
    COLUMNS expired_ts;
DEFINE INDEX old_verify_keys_server_idx ON TABLE old_verify_keys 
    COLUMNS server_name;
