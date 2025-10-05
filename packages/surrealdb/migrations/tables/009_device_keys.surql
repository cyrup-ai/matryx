-- =====================================================
-- Migration: 009
-- Table: device_keys
-- Entity: packages/entity/src/types/device_keys.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Device keys table - E2E encryption device keys (queried by keys.rs)
DEFINE TABLE device_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE  -- Any authenticated user (keys are discoverable for crypto)
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE device_keys TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD device_id ON TABLE device_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithms ON TABLE device_keys TYPE array<string> DEFAULT [];
DEFINE FIELD keys ON TABLE device_keys TYPE object DEFAULT {};
DEFINE FIELD signatures ON TABLE device_keys TYPE object DEFAULT {};
DEFINE FIELD unsigned ON TABLE device_keys TYPE option<object>;
DEFINE FIELD user_signing_key ON TABLE device_keys TYPE option<string>;
DEFINE FIELD device_display_name ON TABLE device_keys TYPE option<string>;

DEFINE INDEX device_keys_user_device_idx ON TABLE device_keys COLUMNS user_id, device_id UNIQUE;
DEFINE INDEX device_keys_user_idx ON TABLE device_keys COLUMNS user_id;
