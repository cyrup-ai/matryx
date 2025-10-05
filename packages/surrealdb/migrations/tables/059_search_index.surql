-- =====================================================
-- Migration: 059
-- Table: search_index
-- Entity: packages/entity/src/types/search_index.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Search index metadata
DEFINE TABLE search_index SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD event_id ON TABLE search_index TYPE string;
DEFINE FIELD room_id ON TABLE search_index TYPE string;
DEFINE FIELD sender ON TABLE search_index TYPE string;
DEFINE FIELD event_type ON TABLE search_index TYPE string;
DEFINE FIELD content_body ON TABLE search_index TYPE option<string>;
DEFINE FIELD content_msgtype ON TABLE search_index TYPE option<string>;
DEFINE FIELD origin_server_ts ON TABLE search_index TYPE datetime;
