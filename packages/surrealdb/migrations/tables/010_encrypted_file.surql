-- =====================================================
-- Migration: 010
-- Table: encrypted_file
-- Entity: packages/entity/src/types/encrypted_file.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

DEFINE TABLE encrypted_file SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.user_id != NONE;  -- Files are accessible to authenticated users

DEFINE FIELD url ON TABLE encrypted_file TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key ON TABLE encrypted_file TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD iv ON TABLE encrypted_file TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD hashes ON TABLE encrypted_file TYPE object DEFAULT {};
DEFINE FIELD v ON TABLE encrypted_file TYPE string ASSERT string::is::not::empty($value);
