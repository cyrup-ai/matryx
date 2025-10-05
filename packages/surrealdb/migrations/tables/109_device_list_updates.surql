-- =====================================================
-- Migration: 109
-- Table: device_list_updates
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/device.rs, packages/surrealdb/src/repository/sync.rs
-- =====================================================

DEFINE TABLE device_list_updates SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE device_list_updates TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE device_list_updates TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD action ON TABLE device_list_updates TYPE string ASSERT $value IN ['added', 'removed', 'left'];
DEFINE FIELD updated_at ON TABLE device_list_updates TYPE datetime DEFAULT time::now();

DEFINE INDEX dlu_user_updated_idx ON TABLE device_list_updates COLUMNS user_id, updated_at;
