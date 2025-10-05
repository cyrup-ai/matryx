-- =====================================================
-- Migration: 060
-- Table: send_join_request
-- Entity: packages/entity/src/types/send_join_request.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- SendJoinRequest table - matches SendJoinRequest entity exactly
DEFINE TABLE send_join_request SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD content ON TABLE send_join_request TYPE object DEFAULT {};
DEFINE FIELD origin ON TABLE send_join_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server_ts ON TABLE send_join_request TYPE int ASSERT $value > 0;
DEFINE FIELD sender ON TABLE send_join_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD state_key ON TABLE send_join_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD event_type ON TABLE send_join_request TYPE string ASSERT string::is::not::empty($value);
