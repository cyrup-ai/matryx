-- =====================================================
-- Migration: 098
-- Table: room_aliases
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/room_alias.rs
-- =====================================================

DEFINE TABLE room_aliases SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD alias ON TABLE room_aliases TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE room_aliases TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD creator ON TABLE room_aliases TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE room_aliases TYPE datetime DEFAULT time::now();

DEFINE INDEX room_aliases_alias_idx ON TABLE room_aliases COLUMNS alias UNIQUE;
DEFINE INDEX room_aliases_room_idx ON TABLE room_aliases COLUMNS room_id;
