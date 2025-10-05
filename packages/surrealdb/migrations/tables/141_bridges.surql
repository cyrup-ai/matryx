-- =====================================================
-- Migration: 141
-- Table: bridges
-- Entity: BridgeConfig (third_party.rs line 44)
-- Repositories: bridge.rs
-- =====================================================

-- Application service bridge configurations
DEFINE TABLE bridges SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD bridge_id ON TABLE bridges TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD protocol ON TABLE bridges TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD name ON TABLE bridges TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD url ON TABLE bridges TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD as_token ON TABLE bridges TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD hs_token ON TABLE bridges TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD status ON TABLE bridges TYPE string 
    ASSERT $value IN ['Active', 'Inactive', 'Error', 'Maintenance'];

DEFINE FIELD created_at ON TABLE bridges TYPE datetime DEFAULT time::now();

DEFINE FIELD last_seen ON TABLE bridges TYPE option<datetime>;

-- Indexes based on repository query patterns (bridge.rs lines 47, 140, 160)
DEFINE INDEX bridges_bridge_id_idx ON TABLE bridges COLUMNS bridge_id UNIQUE;
DEFINE INDEX bridges_protocol_idx ON TABLE bridges COLUMNS protocol;
DEFINE INDEX bridges_status_idx ON TABLE bridges COLUMNS status;
DEFINE INDEX bridges_status_last_seen_idx ON TABLE bridges COLUMNS status, last_seen;
