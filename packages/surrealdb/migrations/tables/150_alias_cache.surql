-- =====================================================
-- Migration: 150
-- Table: alias_cache
-- Entity: Room alias lookup cache
-- Repositories: room_alias.rs
-- =====================================================

-- Room alias lookup cache for performance
DEFINE TABLE alias_cache SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update WHERE $auth.user_id != NONE OR $auth.server_name != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD cache_key ON TABLE alias_cache TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD cache_value ON TABLE alias_cache TYPE object DEFAULT {};

DEFINE FIELD expires_at ON TABLE alias_cache TYPE datetime;

DEFINE FIELD created_at ON TABLE alias_cache TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (room_alias.rs line 452)
DEFINE INDEX alias_cache_key_idx ON TABLE alias_cache COLUMNS cache_key UNIQUE;
DEFINE INDEX alias_cache_expires_idx ON TABLE alias_cache COLUMNS expires_at;
