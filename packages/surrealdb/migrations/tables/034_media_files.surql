-- =====================================================
-- Migration: 034
-- Table: media_files
-- Entity: packages/entity/src/types/media_files.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Media files table for media storage
DEFINE TABLE media_files SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.user_id = uploaded_by OR $auth.is_admin = true;

DEFINE FIELD media_id ON TABLE media_files TYPE string ASSERT $value != "";
DEFINE FIELD server_name ON TABLE media_files TYPE string ASSERT $value != "";
DEFINE FIELD content_type ON TABLE media_files TYPE string ASSERT $value != "";
DEFINE FIELD content_length ON TABLE media_files TYPE int ASSERT $value > 0;
DEFINE FIELD file_path ON TABLE media_files TYPE string ASSERT $value != "";
DEFINE FIELD upload_name ON TABLE media_files TYPE option<string>;
DEFINE FIELD uploaded_by ON TABLE media_files TYPE string ASSERT $value != "";
DEFINE FIELD created_at ON TABLE media_files TYPE datetime DEFAULT time::now();
