-- =====================================================
-- Migration: 147
-- Table: media_info
-- Entity: MediaInfo (media.rs line 8)
-- Repositories: media.rs
-- =====================================================

-- Media file metadata
DEFINE TABLE media_info SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD media_id ON TABLE media_info TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD server_name ON TABLE media_info TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD content_type ON TABLE media_info TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD content_length ON TABLE media_info TYPE int;

DEFINE FIELD upload_name ON TABLE media_info TYPE option<string>;

DEFINE FIELD uploaded_by ON TABLE media_info TYPE string
    ASSERT string::is::not::empty($value);

DEFINE FIELD created_at ON TABLE media_info TYPE datetime DEFAULT time::now();

DEFINE FIELD expires_at ON TABLE media_info TYPE option<datetime>;

DEFINE FIELD content_hash ON TABLE media_info TYPE option<string>;

-- Indexes based on repository query patterns (media.rs lines 34, 234, 334)
DEFINE INDEX media_info_media_server_idx ON TABLE media_info COLUMNS media_id, server_name UNIQUE;
DEFINE INDEX media_info_content_hash_idx ON TABLE media_info COLUMNS content_hash;
DEFINE INDEX media_info_expires_idx ON TABLE media_info COLUMNS expires_at;
