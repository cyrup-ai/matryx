-- =====================================================
-- Migration: 090
-- Table: backup_version
-- Entity: Based on BackupVersionRecord struct
-- Repositories: key_backup.rs
-- =====================================================

-- E2E encryption key backup version metadata
DEFINE TABLE backup_version SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE backup_version TYPE string ASSERT string::is::not::empty($value);

DEFINE FIELD user_id ON TABLE backup_version TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD version ON TABLE backup_version TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm ON TABLE backup_version TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD auth_data ON TABLE backup_version TYPE object DEFAULT {};
DEFINE FIELD count ON TABLE backup_version TYPE int DEFAULT 0;
DEFINE FIELD etag ON TABLE backup_version TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE backup_version TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE backup_version TYPE datetime DEFAULT time::now();

DEFINE INDEX backup_version_user_idx ON TABLE backup_version 
    COLUMNS user_id;
DEFINE INDEX backup_version_user_version_idx ON TABLE backup_version 
    COLUMNS user_id, version UNIQUE;
DEFINE INDEX backup_version_created_idx ON TABLE backup_version 
    COLUMNS created_at;
