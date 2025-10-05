-- =====================================================
-- Migration: 072
-- Table: thirdparty_protocols
-- Entity: packages/entity/src/types/thirdparty_protocols.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Third-party protocols
DEFINE TABLE thirdparty_protocols SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD protocol_id ON TABLE thirdparty_protocols TYPE string;
DEFINE FIELD display_name ON TABLE thirdparty_protocols TYPE string;
DEFINE FIELD avatar_url ON TABLE thirdparty_protocols TYPE option<string>;
DEFINE FIELD user_fields ON TABLE thirdparty_protocols TYPE array<object>;
DEFINE FIELD location_fields ON TABLE thirdparty_protocols TYPE array<object>;
DEFINE FIELD instances ON TABLE thirdparty_protocols TYPE array<object>;
DEFINE FIELD created_at ON TABLE thirdparty_protocols TYPE datetime DEFAULT time::now();
