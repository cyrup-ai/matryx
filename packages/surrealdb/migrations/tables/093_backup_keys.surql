-- =====================================================
-- Migration: 093
-- Table: backup_keys
-- Entity: Based on RoomKeyBackupRecord struct
-- Repositories: key_backup.rs
-- =====================================================

-- Store encrypted room keys in user's E2E key backup
DEFINE TABLE backup_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD id ON TABLE backup_keys TYPE string ASSERT string::is::not::empty($value);

DEFINE FIELD user_id ON TABLE backup_keys TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD version ON TABLE backup_keys TYPE string ASSERT string::is::not::empty($value);

DEFINE FIELD room_id ON TABLE backup_keys TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '!') 
    AND string::contains($value, ':');

DEFINE FIELD session_id ON TABLE backup_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_data ON TABLE backup_keys TYPE object ASSERT $value != NONE;
DEFINE FIELD created_at ON TABLE backup_keys TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE backup_keys TYPE datetime DEFAULT time::now();

DEFINE INDEX backup_keys_user_version_idx ON TABLE backup_keys 
    COLUMNS user_id, version;
DEFINE INDEX backup_keys_user_version_room_idx ON TABLE backup_keys 
    COLUMNS user_id, version, room_id;
DEFINE INDEX backup_keys_user_version_room_session_idx ON TABLE backup_keys 
    COLUMNS user_id, version, room_id, session_id UNIQUE;
