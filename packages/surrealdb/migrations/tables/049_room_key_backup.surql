-- =====================================================
-- Migration: 049
-- Table: room_key_backup
-- Entity: packages/entity/src/types/room_key_backup.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

DEFINE TABLE room_key_backup SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD version ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD auth_data ON TABLE room_key_backup TYPE object DEFAULT {};
DEFINE FIELD count ON TABLE room_key_backup TYPE int DEFAULT 0;
DEFINE FIELD etag ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);
