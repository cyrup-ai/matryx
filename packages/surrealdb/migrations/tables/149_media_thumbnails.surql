-- =====================================================
-- Migration: 149
-- Table: media_thumbnails
-- Entity: Media thumbnail storage
-- Repositories: media.rs
-- =====================================================

-- Media thumbnail storage (base64 encoded)
DEFINE TABLE media_thumbnails SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD media_id ON TABLE media_thumbnails TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD server_name ON TABLE media_thumbnails TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD width ON TABLE media_thumbnails TYPE int;

DEFINE FIELD height ON TABLE media_thumbnails TYPE int;

DEFINE FIELD method ON TABLE media_thumbnails TYPE string 
    ASSERT $value IN ['crop', 'scale'];

DEFINE FIELD thumbnail_data ON TABLE media_thumbnails TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD created_at ON TABLE media_thumbnails TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (media.rs line 138)
DEFINE INDEX media_thumbnails_media_server_size_method_idx ON TABLE media_thumbnails COLUMNS media_id, server_name, width, height, method UNIQUE;
