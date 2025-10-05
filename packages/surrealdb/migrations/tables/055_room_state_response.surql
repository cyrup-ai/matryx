-- =====================================================
-- Migration: 055
-- Table: room_state_response
-- Entity: packages/entity/src/types/room_state_response.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

DEFINE TABLE room_state_response SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD auth_chain ON TABLE room_state_response TYPE array<object> DEFAULT [];
DEFINE FIELD pdus ON TABLE room_state_response TYPE array<object> DEFAULT [];
DEFINE FIELD room_id ON TABLE room_state_response TYPE string ASSERT string::is::not::empty($value);
