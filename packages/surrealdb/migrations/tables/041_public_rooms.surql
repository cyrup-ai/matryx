-- =====================================================
-- Migration: 041
-- Table: public_rooms
-- Entity: packages/entity/src/types/public_rooms.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Public rooms directory
DEFINE TABLE public_rooms SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true OR $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.room_id AND membership = 'join');

DEFINE FIELD room_id ON TABLE public_rooms TYPE string;
DEFINE FIELD name ON TABLE public_rooms TYPE option<string>;
DEFINE FIELD topic ON TABLE public_rooms TYPE option<string>;
DEFINE FIELD canonical_alias ON TABLE public_rooms TYPE option<string>;
DEFINE FIELD num_joined_members ON TABLE public_rooms TYPE int DEFAULT 0;
DEFINE FIELD avatar_url ON TABLE public_rooms TYPE option<string>;
DEFINE FIELD world_readable ON TABLE public_rooms TYPE bool DEFAULT false;
DEFINE FIELD guest_can_join ON TABLE public_rooms TYPE bool DEFAULT false;
DEFINE FIELD join_rule ON TABLE public_rooms TYPE string DEFAULT 'invite';
DEFINE FIELD room_type ON TABLE public_rooms TYPE option<string>;
DEFINE FIELD created_at ON TABLE public_rooms TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE public_rooms TYPE datetime DEFAULT time::now();
