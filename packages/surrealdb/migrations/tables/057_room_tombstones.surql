-- =====================================================
-- Migration: 057
-- Table: room_tombstones
-- Entity: packages/entity/src/types/room_tombstones.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room upgrades/tombstones
DEFINE TABLE room_tombstones SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.old_room_id AND membership = 'join');

DEFINE FIELD old_room_id ON TABLE room_tombstones TYPE string ASSERT $value != "";
DEFINE FIELD new_room_id ON TABLE room_tombstones TYPE string ASSERT $value != "";
DEFINE FIELD upgrade_version ON TABLE room_tombstones TYPE string ASSERT $value != "";
DEFINE FIELD created_by ON TABLE room_tombstones TYPE string ASSERT $value != "";
DEFINE FIELD created_at ON TABLE room_tombstones TYPE datetime DEFAULT time::now();
