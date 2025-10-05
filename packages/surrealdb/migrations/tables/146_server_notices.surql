-- =====================================================
-- Migration: 146
-- Table: server_notices
-- Entity: ServerNotice (server_notices.rs line 7)
-- Repositories: server_notices.rs
-- =====================================================

-- Server administrative notices to users
DEFINE TABLE server_notices SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.admin = true
        FOR update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD notice_id ON TABLE server_notices TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD user_id ON TABLE server_notices TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD notice_type ON TABLE server_notices TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD content ON TABLE server_notices TYPE object DEFAULT {};

DEFINE FIELD created_at ON TABLE server_notices TYPE datetime DEFAULT time::now();

DEFINE FIELD read_at ON TABLE server_notices TYPE option<datetime>;

DEFINE FIELD is_read ON TABLE server_notices TYPE bool DEFAULT false;

-- Indexes based on repository query patterns (server_notices.rs lines 259, 353, 372)
DEFINE INDEX server_notices_notice_id_idx ON TABLE server_notices COLUMNS notice_id UNIQUE;
DEFINE INDEX server_notices_user_created_idx ON TABLE server_notices COLUMNS user_id, created_at;
DEFINE INDEX server_notices_user_read_idx ON TABLE server_notices COLUMNS user_id, is_read;
DEFINE INDEX server_notices_created_read_idx ON TABLE server_notices COLUMNS created_at, is_read;
