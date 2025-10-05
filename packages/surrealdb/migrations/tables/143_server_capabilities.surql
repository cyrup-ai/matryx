-- =====================================================
-- Migration: 143
-- Table: server_capabilities
-- Entity: Server capability flags
-- Repositories: capabilities.rs
-- =====================================================

-- Server capability configuration
DEFINE TABLE server_capabilities SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD change_password ON TABLE server_capabilities TYPE bool DEFAULT true;

DEFINE FIELD set_displayname ON TABLE server_capabilities TYPE bool DEFAULT true;

DEFINE FIELD set_avatar_url ON TABLE server_capabilities TYPE bool DEFAULT true;

DEFINE FIELD threepid_changes ON TABLE server_capabilities TYPE bool DEFAULT true;

DEFINE FIELD room_versions ON TABLE server_capabilities TYPE object DEFAULT {};

DEFINE FIELD federation_enabled ON TABLE server_capabilities TYPE bool DEFAULT true;

DEFINE FIELD registration_enabled ON TABLE server_capabilities TYPE bool DEFAULT false;

DEFINE FIELD updated_at ON TABLE server_capabilities TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (capabilities.rs line 95)
DEFINE INDEX server_capabilities_updated_idx ON TABLE server_capabilities COLUMNS updated_at;
