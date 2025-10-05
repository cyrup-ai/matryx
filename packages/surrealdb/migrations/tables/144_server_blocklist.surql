-- =====================================================
-- Migration: 144
-- Table: server_blocklist
-- Entity: Federation server blocklist
-- Repositories: federation.rs
-- =====================================================

-- Federation server blocklist for access control
DEFINE TABLE server_blocklist SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.server_name != NONE
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD server_name ON TABLE server_blocklist TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD blocked ON TABLE server_blocklist TYPE bool DEFAULT true;

DEFINE FIELD reason ON TABLE server_blocklist TYPE option<string>;

DEFINE FIELD third_party_invites ON TABLE server_blocklist TYPE bool DEFAULT false;

DEFINE FIELD blocked_at ON TABLE server_blocklist TYPE datetime DEFAULT time::now();

DEFINE FIELD blocked_by ON TABLE server_blocklist TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

-- Indexes based on repository query patterns (federation.rs line 1357)
DEFINE INDEX server_blocklist_server_idx ON TABLE server_blocklist COLUMNS server_name UNIQUE;
DEFINE INDEX server_blocklist_blocked_idx ON TABLE server_blocklist COLUMNS blocked;
