-- =====================================================
-- Migration: 103
-- Table: to_device_events
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/device.rs
-- =====================================================

DEFINE TABLE to_device_events SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id = user_id
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.user_id = user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE to_device_events TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD device_id ON TABLE to_device_events TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sender ON TABLE to_device_events TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD message_type ON TABLE to_device_events TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD content ON TABLE to_device_events TYPE object DEFAULT {};
DEFINE FIELD delivered ON TABLE to_device_events TYPE bool DEFAULT false;
DEFINE FIELD created_at ON TABLE to_device_events TYPE datetime DEFAULT time::now();

DEFINE INDEX tde_user_device_idx ON TABLE to_device_events COLUMNS user_id, device_id;
DEFINE INDEX tde_user_device_delivered_idx ON TABLE to_device_events COLUMNS user_id, device_id, delivered, created_at;
DEFINE INDEX tde_created_idx ON TABLE to_device_events COLUMNS created_at;
