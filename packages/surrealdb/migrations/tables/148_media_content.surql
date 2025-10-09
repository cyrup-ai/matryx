-- =====================================================
-- Migration: 148
-- Table: media_content
-- Entity: Media file content (binary storage)
-- Repositories: media.rs
-- =====================================================

-- Media file content storage (base64 encoded)
DEFINE TABLE media_content SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD media_id ON TABLE media_content TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD server_name ON TABLE media_content TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD content ON TABLE media_content TYPE string 
    ASSERT string::is::not::empty($value);
    -- Stores base64-encoded binary content

DEFINE FIELD content_type ON TABLE media_content TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD content_length ON TABLE media_content TYPE int;

DEFINE FIELD created_at ON TABLE media_content TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (media.rs line 68)
DEFINE INDEX media_content_media_server_idx ON TABLE media_content COLUMNS media_id, server_name UNIQUE;
