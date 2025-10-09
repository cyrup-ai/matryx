-- =====================================================
-- Migration: 108
-- Table: room_hierarchy
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/federation_service.rs
-- =====================================================

DEFINE TABLE room_hierarchy SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD parent_room_id ON TABLE room_hierarchy TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD child_room_id ON TABLE room_hierarchy TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD order ON TABLE room_hierarchy TYPE option<string>;
DEFINE FIELD via ON TABLE room_hierarchy TYPE array<string> DEFAULT [];
DEFINE FIELD suggested ON TABLE room_hierarchy TYPE bool DEFAULT false;
DEFINE FIELD created_at ON TABLE room_hierarchy TYPE datetime DEFAULT time::now();

DEFINE INDEX rh_parent_idx ON TABLE room_hierarchy COLUMNS parent_room_id;
DEFINE INDEX rh_child_idx ON TABLE room_hierarchy COLUMNS child_room_id;
DEFINE INDEX rh_parent_child_idx ON TABLE room_hierarchy COLUMNS parent_room_id, child_room_id UNIQUE;
