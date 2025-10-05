-- =====================================================
-- Migration: 056
-- Table: room_tags
-- Entity: packages/entity/src/types/room_tags.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room tags table for room organization
DEFINE TABLE room_tags SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.user_id = user_id OR $auth.is_admin = true
        FOR create, update, delete WHERE $auth.user_id = user_id OR $auth.is_admin = true;

DEFINE FIELD id ON TABLE room_tags TYPE string ASSERT $value != "";
DEFINE FIELD user_id ON TABLE room_tags TYPE string ASSERT $value != "";
DEFINE FIELD room_id ON TABLE room_tags TYPE string ASSERT $value != "";
DEFINE FIELD tag ON TABLE room_tags TYPE string ASSERT $value != "";
DEFINE FIELD tag_order ON TABLE room_tags TYPE option<float>;
DEFINE FIELD created_at ON TABLE room_tags TYPE datetime DEFAULT time::now();
