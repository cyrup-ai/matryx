-- =====================================================
-- Migration: 020
-- Table: invite_v1_request
-- Entity: packages/entity/src/types/invite_v1_request.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- InviteV1Request table - matches InviteV1Request entity exactly
DEFINE TABLE invite_v1_request SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD content ON TABLE invite_v1_request TYPE object DEFAULT {};
DEFINE FIELD origin ON TABLE invite_v1_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server_ts ON TABLE invite_v1_request TYPE int ASSERT $value > 0;
DEFINE FIELD sender ON TABLE invite_v1_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD state_key ON TABLE invite_v1_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD event_type ON TABLE invite_v1_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD unsigned ON TABLE invite_v1_request TYPE object DEFAULT {};
