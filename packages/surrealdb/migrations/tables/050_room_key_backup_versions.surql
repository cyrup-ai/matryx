-- =====================================================
-- Migration: 050
-- Table: room_key_backup_versions
-- Entity: packages/entity/src/types/room_key_backup_versions.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room key backup versions table - E2E encryption room key backup versions (queried by room_keys.rs)
DEFINE TABLE room_key_backup_versions SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE room_key_backup_versions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD version ON TABLE room_key_backup_versions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm ON TABLE room_key_backup_versions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD auth_data ON TABLE room_key_backup_versions TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE room_key_backup_versions TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE room_key_backup_versions TYPE datetime DEFAULT time::now();
DEFINE FIELD deleted ON TABLE room_key_backup_versions TYPE bool DEFAULT false;
