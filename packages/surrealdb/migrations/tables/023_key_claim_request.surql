-- =====================================================
-- Migration: 023
-- Table: key_claim_request
-- Entity: packages/entity/src/types/key_claim_request.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- KeyClaimRequest table - matches KeyClaimRequest entity exactly
DEFINE TABLE key_claim_request SCHEMAFULL
    PERMISSIONS
        FOR select, create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD one_time_keys ON TABLE key_claim_request TYPE object DEFAULT {};
DEFINE FIELD requesting_user_id ON TABLE key_claim_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD timestamp ON TABLE key_claim_request TYPE datetime DEFAULT time::now();
