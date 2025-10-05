-- =====================================================
-- Migration: 083
-- Table: user_profiles
-- Entity: packages/entity/src/types/user_profiles.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- User profiles table for User Experience API
DEFINE TABLE user_profiles SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.user_id = user_id OR $auth.is_admin = true
        FOR create, update, delete WHERE $auth.user_id = user_id OR $auth.is_admin = true;

DEFINE FIELD user_id ON TABLE user_profiles TYPE string ASSERT $value != "";
DEFINE FIELD display_name ON TABLE user_profiles TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE user_profiles TYPE option<string>;
DEFINE FIELD created_at ON TABLE user_profiles TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE user_profiles TYPE datetime DEFAULT time::now();
