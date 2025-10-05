-- =====================================================
-- Migration: 070
-- Table: third_party_sessions
-- Entity: packages/entity/src/types/third_party_sessions.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Third-party identifier validation sessions
DEFINE TABLE third_party_sessions SCHEMAFULL
    PERMISSIONS
        FOR select, create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD session_id ON TABLE third_party_sessions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD client_secret ON TABLE third_party_sessions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD medium ON TABLE third_party_sessions TYPE string ASSERT $value IN ["email", "msisdn"];
DEFINE FIELD address ON TABLE third_party_sessions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD verification_token ON TABLE third_party_sessions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD verified ON TABLE third_party_sessions TYPE bool DEFAULT false;
DEFINE FIELD expires_at ON TABLE third_party_sessions TYPE int ASSERT $value > 0;
DEFINE FIELD user_id ON TABLE third_party_sessions TYPE option<string>;
DEFINE FIELD created_at ON TABLE third_party_sessions TYPE int DEFAULT time::now();
