-- =====================================================
-- Migration: 062
-- Table: server_config
-- Entity: packages/entity/src/types/server_config.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Server capabilities and configuration
DEFINE TABLE server_config SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true -- Public server information
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD key ON TABLE server_config TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD value ON TABLE server_config TYPE flexible; -- Can be string, number, boolean, object, array
DEFINE FIELD description ON TABLE server_config TYPE option<string>;
DEFINE FIELD updated_at ON TABLE server_config TYPE datetime DEFAULT time::now();
DEFINE FIELD is_public ON TABLE server_config TYPE bool DEFAULT false; -- Whether exposed in server discovery
