-- =====================================================
-- Migration: 091
-- Table: server_keys
-- Entity: packages/entity/src/types/server_keys_response.rs
-- Repositories: key_server.rs, federation.rs
-- =====================================================

-- Cache federation server signing keys
DEFINE TABLE server_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update WHERE $auth.server_name != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD server_name ON TABLE server_keys TYPE string 
    ASSERT string::is::not::empty($value) 
    AND (string::contains($value, ':') OR string::contains($value, '.'));

DEFINE FIELD verify_keys ON TABLE server_keys TYPE object ASSERT $value != NONE;
DEFINE FIELD old_verify_keys ON TABLE server_keys TYPE option<object>;
DEFINE FIELD valid_until_ts ON TABLE server_keys TYPE int ASSERT $value > 0;
DEFINE FIELD signatures ON TABLE server_keys TYPE object DEFAULT {};
DEFINE FIELD cached_at ON TABLE server_keys TYPE datetime DEFAULT time::now();
DEFINE FIELD key_ids ON TABLE server_keys TYPE option<array<string>>;

DEFINE INDEX server_keys_server_idx ON TABLE server_keys 
    COLUMNS server_name;
DEFINE INDEX server_keys_valid_idx ON TABLE server_keys 
    COLUMNS valid_until_ts;
DEFINE INDEX server_keys_server_valid_idx ON TABLE server_keys 
    COLUMNS server_name, valid_until_ts;
