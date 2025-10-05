-- =====================================================
-- Migration: 061
-- Table: send_join_room_state
-- Entity: packages/entity/src/types/send_join_room_state.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- SendJoinRoomState table - matches SendJoinRoomState entity exactly
DEFINE TABLE send_join_room_state SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD state ON TABLE send_join_room_state TYPE array<object> DEFAULT [];
DEFINE FIELD auth_chain ON TABLE send_join_room_state TYPE array<object> DEFAULT [];
