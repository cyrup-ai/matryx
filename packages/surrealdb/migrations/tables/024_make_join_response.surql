-- =====================================================
-- Migration: 024
-- Table: make_join_response
-- Entity: packages/entity/src/types/make_join_response.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- MakeJoinResponse table - matches MakeJoinResponse entity exactly
DEFINE TABLE make_join_response SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD event ON TABLE make_join_response TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD room_version ON TABLE make_join_response TYPE string ASSERT string::is::not::empty($value);
