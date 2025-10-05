-- =====================================================
-- Migration: 021
-- Table: invite_v2_request
-- Entity: packages/entity/src/types/invite_v2_request.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- =====================================================
-- BATCH 2: FEDERATION ENTITIES
-- =====================================================-- InviteV2Request table - matches InviteV2Request entity exactly
DEFINE TABLE invite_v2_request SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD event ON TABLE invite_v2_request TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD invite_room_state ON TABLE invite_v2_request TYPE option<array<object>>;
DEFINE FIELD room_version ON TABLE invite_v2_request TYPE string ASSERT string::is::not::empty($value);
