-- =====================================================
-- Migration: 058
-- Table: rooms
-- Entity: packages/entity/src/types/rooms.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Rooms table - Alternative room storage (queried by membership.rs and other repositories)
DEFINE TABLE rooms SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update WHERE creator = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE rooms TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD creator ON TABLE rooms TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE rooms TYPE datetime DEFAULT time::now();
DEFINE FIELD room_version ON TABLE rooms TYPE string DEFAULT '10';
DEFINE FIELD name ON TABLE rooms TYPE option<string>;
DEFINE FIELD topic ON TABLE rooms TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE rooms TYPE option<string>;
DEFINE FIELD visibility ON TABLE rooms TYPE string DEFAULT 'private';
