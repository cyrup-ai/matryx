-- =====================================================
-- Migration: 066
-- Table: session
-- Entity: packages/entity/src/types/session.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Session table - Matrix session management (matches Session entity from packages/entity/src/types/session.rs)
DEFINE TABLE session SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.user_id != NONE
        FOR update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD session_id ON TABLE session TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE session TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD device_id ON TABLE session TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD access_token ON TABLE session TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD refresh_token ON TABLE session TYPE option<string>;
DEFINE FIELD created_at ON TABLE session TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE session TYPE option<datetime>;
DEFINE FIELD last_seen ON TABLE session TYPE option<datetime>;
DEFINE FIELD last_used_at ON TABLE session TYPE option<datetime>;
DEFINE FIELD last_used_ip ON TABLE session TYPE option<string>;
DEFINE FIELD user_agent ON TABLE session TYPE option<string>;
DEFINE FIELD is_active ON TABLE session TYPE bool DEFAULT true;
DEFINE FIELD valid ON TABLE session TYPE bool DEFAULT true;
DEFINE FIELD puppets_user_id ON TABLE session TYPE option<string>;
DEFINE FIELD updated_at ON TABLE session TYPE datetime DEFAULT time::now();

DEFINE INDEX session_id_idx ON TABLE session COLUMNS session_id UNIQUE;
DEFINE INDEX session_access_token_idx ON TABLE session COLUMNS access_token UNIQUE;
DEFINE INDEX session_user_idx ON TABLE session COLUMNS user_id;
DEFINE INDEX session_user_active_idx ON TABLE session COLUMNS user_id, is_active;
DEFINE INDEX session_device_idx ON TABLE session COLUMNS device_id;
