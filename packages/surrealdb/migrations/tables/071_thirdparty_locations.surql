-- =====================================================
-- Migration: 071
-- Table: thirdparty_locations
-- Entity: packages/entity/src/types/thirdparty_locations.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Third-party locations (rooms/channels)
DEFINE TABLE thirdparty_locations SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD protocol ON TABLE thirdparty_locations TYPE string;
DEFINE FIELD alias ON TABLE thirdparty_locations TYPE string;
DEFINE FIELD fields ON TABLE thirdparty_locations TYPE object;
DEFINE FIELD created_at ON TABLE thirdparty_locations TYPE datetime DEFAULT time::now();
