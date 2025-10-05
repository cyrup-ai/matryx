-- =====================================================
-- Migration: 121
-- Table: third_party_invite_log
-- Entity: Third-party invite tracking
-- Repositories: federation.rs
-- =====================================================

-- Third-party invite tracking for rate limiting and auditing
DEFINE TABLE third_party_invite_log SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.server_name != NONE
        FOR create WHERE $auth.server_name != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD server_name ON TABLE third_party_invite_log TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD room_id ON TABLE third_party_invite_log TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '!') 
    AND string::contains($value, ':');

DEFINE FIELD inviter ON TABLE third_party_invite_log TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD medium ON TABLE third_party_invite_log TYPE string 
    ASSERT $value IN ['email', 'msisdn'];

DEFINE FIELD address ON TABLE third_party_invite_log TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD timestamp ON TABLE third_party_invite_log TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns
DEFINE INDEX third_party_invite_log_server_timestamp_idx ON TABLE third_party_invite_log COLUMNS server_name, timestamp;
DEFINE INDEX third_party_invite_log_room_idx ON TABLE third_party_invite_log COLUMNS room_id;
