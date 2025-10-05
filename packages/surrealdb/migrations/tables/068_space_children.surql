-- =====================================================
-- Migration: 068
-- Table: space_children
-- Entity: packages/entity/src/types/space_children.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Space relationships
DEFINE TABLE space_children SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.parent_room_id AND membership = 'join');

DEFINE FIELD parent_room_id ON TABLE space_children TYPE string ASSERT $value != "";
DEFINE FIELD child_room_id ON TABLE space_children TYPE string ASSERT $value != "";
DEFINE FIELD order_value ON TABLE space_children TYPE option<string>;
DEFINE FIELD suggested ON TABLE space_children TYPE bool DEFAULT false;
DEFINE FIELD via ON TABLE space_children TYPE array<string>;
DEFINE FIELD created_at ON TABLE space_children TYPE datetime DEFAULT time::now();
