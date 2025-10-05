-- =====================================================
-- Migration: 003
-- Table: backfill_response
-- Entity: packages/entity/src/types/backfill_response.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- BackfillResponse table - matches BackfillResponse entity exactly
DEFINE TABLE backfill_response SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD origin ON TABLE backfill_response TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server_ts ON TABLE backfill_response TYPE int ASSERT $value > 0;
DEFINE FIELD pdus ON TABLE backfill_response TYPE array<object> DEFAULT [];
