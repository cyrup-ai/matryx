-- =====================================================
-- Migration: 138
-- Table: federation_request_log
-- Entity: Federation request tracking
-- Repositories: federation.rs
-- =====================================================

-- Federation request log for replay protection and auditing
DEFINE TABLE federation_request_log SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE $auth.server_name != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD origin ON TABLE federation_request_log TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD destination ON TABLE federation_request_log TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD request_id ON TABLE federation_request_log TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD created_at ON TABLE federation_request_log TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (federation.rs lines 224, 498)
DEFINE INDEX federation_request_log_origin_request_idx ON TABLE federation_request_log COLUMNS origin, request_id;
DEFINE INDEX federation_request_log_created_idx ON TABLE federation_request_log COLUMNS created_at;
DEFINE INDEX federation_request_log_origin_dest_idx ON TABLE federation_request_log COLUMNS origin, destination;
