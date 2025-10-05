-- =====================================================
-- Migration: 145
-- Table: server_federation_config
-- Entity: Federation configuration per server
-- Repositories: federation.rs
-- =====================================================

-- Federation configuration and ACL per server
DEFINE TABLE server_federation_config SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.server_name != NONE
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD server_name ON TABLE server_federation_config TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD federation_enabled ON TABLE server_federation_config TYPE bool DEFAULT true;

DEFINE FIELD allow_list ON TABLE server_federation_config TYPE array<string> DEFAULT [];

DEFINE FIELD deny_list ON TABLE server_federation_config TYPE array<string> DEFAULT [];

DEFINE FIELD updated_at ON TABLE server_federation_config TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (federation.rs line 1373)
DEFINE INDEX server_federation_config_server_idx ON TABLE server_federation_config COLUMNS server_name UNIQUE;
