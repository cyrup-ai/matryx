-- =====================================================
-- Migration: 085
-- Table: user_search_index
-- Entity: packages/entity/src/types/user_search_index.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- User search index
DEFINE TABLE user_search_index SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD user_id ON TABLE user_search_index TYPE string ASSERT $value != "";
DEFINE FIELD display_name ON TABLE user_search_index TYPE option<string>;
DEFINE FIELD search_vector ON TABLE user_search_index TYPE option<string>;
DEFINE FIELD last_active ON TABLE user_search_index TYPE option<datetime>;
DEFINE FIELD updated_at ON TABLE user_search_index TYPE datetime DEFAULT time::now();
