-- =====================================================
-- Migration: 008
-- Table: device_key
-- Entity: packages/entity/src/types/device_key.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- DeviceKey table - matches DeviceKey entity exactly
DEFINE TABLE device_key SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE  -- Any authenticated user (keys are discoverable for crypto)
        FOR create, update, delete WHERE $auth.user_id != NONE;  -- Users manage their own device keys

DEFINE FIELD user_id ON TABLE device_key TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD device_id ON TABLE device_key TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithms ON TABLE device_key TYPE array<string> DEFAULT [];
DEFINE FIELD keys ON TABLE device_key TYPE object DEFAULT {};
DEFINE FIELD signatures ON TABLE device_key TYPE object DEFAULT {};
DEFINE FIELD unsigned ON TABLE device_key TYPE option<object>;
