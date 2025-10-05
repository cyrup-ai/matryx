-- =====================================================
-- Migration: 088
-- Table: verification_start
-- Entity: packages/entity/src/types/verification_start.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- VerificationStart table - matches VerificationStart entity exactly
DEFINE TABLE verification_start SCHEMAFULL
    PERMISSIONS
        FOR select WHERE from_user_id = $auth.user_id OR to_user_id = $auth.user_id
        FOR create WHERE from_user_id = $auth.user_id
        FOR update, delete WHERE from_user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD from_device ON TABLE verification_start TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD from_user_id ON TABLE verification_start TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD to_user_id ON TABLE verification_start TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD m_relates_to ON TABLE verification_start TYPE option<object>;
DEFINE FIELD method ON TABLE verification_start TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD next_method ON TABLE verification_start TYPE option<string>;
DEFINE FIELD transaction_id ON TABLE verification_start TYPE option<string>;
