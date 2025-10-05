-- =====================================================
-- Migration: 048
-- Table: room_alias_mapping
-- Entity: packages/entity/src/types/room_alias_mapping.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- RoomAliasMapping table - matches RoomAliasMapping entity exactly
DEFINE TABLE room_alias_mapping SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Room aliases are publicly discoverable
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE room_alias_mapping TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD alias ON TABLE room_alias_mapping TYPE string ASSERT string::is::not::empty($value);
