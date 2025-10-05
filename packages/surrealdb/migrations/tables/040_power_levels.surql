-- =====================================================
-- Migration: 040
-- Table: power_levels
-- Entity: packages/entity/src/types/power_levels.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Power levels table - Room power levels (queried by membership.rs)
DEFINE TABLE power_levels SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE power_levels TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE power_levels TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD power_level ON TABLE power_levels TYPE int DEFAULT 0;
DEFINE FIELD updated_at ON TABLE power_levels TYPE datetime DEFAULT time::now();
