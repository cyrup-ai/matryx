-- =====================================================
-- Migration: 063
-- Table: server_info
-- Entity: packages/entity/src/types/server_info.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- ServerInfo table - matches ServerInfo entity exactly
DEFINE TABLE server_info SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Server info is publicly accessible
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD server ON TABLE server_info TYPE object ASSERT object::len($value) > 0;
