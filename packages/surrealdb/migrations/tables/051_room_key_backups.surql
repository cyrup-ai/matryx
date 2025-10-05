-- =====================================================
-- Migration: 051
-- Table: room_key_backups
-- Entity: packages/entity/src/types/room_key_backups.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room key backups table - E2E encryption room key backups (queried by room_keys.rs, key_backup.rs)
DEFINE TABLE room_key_backups SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE room_key_backups TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD version ON TABLE room_key_backups TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE room_key_backups TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD session_id ON TABLE room_key_backups TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD first_message_index ON TABLE room_key_backups TYPE int;
DEFINE FIELD forwarded_count ON TABLE room_key_backups TYPE int;
DEFINE FIELD is_verified ON TABLE room_key_backups TYPE bool;
DEFINE FIELD session_data ON TABLE room_key_backups TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE room_key_backups TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE room_key_backups TYPE datetime DEFAULT time::now();
DEFINE FIELD backup_version ON TABLE room_key_backups TYPE string;
