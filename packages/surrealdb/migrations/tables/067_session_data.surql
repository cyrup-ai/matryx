-- =====================================================
-- Migration: 067
-- Table: session_data
-- Entity: packages/entity/src/types/session_data.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- SessionData table - matches SessionData entity exactly
DEFINE TABLE session_data SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE session_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD session_id ON TABLE session_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD encrypted_data ON TABLE session_data TYPE object DEFAULT {};
