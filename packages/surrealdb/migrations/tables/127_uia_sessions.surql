-- =====================================================
-- Migration: 127
-- Table: uia_sessions
-- Entity: UiaSession (repository struct)
-- Repositories: uia.rs
-- =====================================================

-- User-Interactive Authentication session tracking
DEFINE TABLE uia_sessions SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE true
        FOR update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD session_id ON TABLE uia_sessions TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD user_id ON TABLE uia_sessions TYPE option<string>;

DEFINE FIELD device_id ON TABLE uia_sessions TYPE option<string>;

DEFINE FIELD flows ON TABLE uia_sessions TYPE array<object> DEFAULT [];

DEFINE FIELD completed_stages ON TABLE uia_sessions TYPE array<string> DEFAULT [];

DEFINE FIELD current_stage ON TABLE uia_sessions TYPE option<string>;

DEFINE FIELD auth_data ON TABLE uia_sessions TYPE object DEFAULT {};

DEFINE FIELD params ON TABLE uia_sessions TYPE object DEFAULT {};

DEFINE FIELD created_at ON TABLE uia_sessions TYPE datetime DEFAULT time::now();

DEFINE FIELD expires_at ON TABLE uia_sessions TYPE datetime;

DEFINE FIELD completed ON TABLE uia_sessions TYPE bool DEFAULT false;

-- Indexes based on repository query patterns
DEFINE INDEX uia_sessions_session_id_idx ON TABLE uia_sessions COLUMNS session_id UNIQUE;
DEFINE INDEX uia_sessions_user_expires_idx ON TABLE uia_sessions COLUMNS user_id, expires_at;
DEFINE INDEX uia_sessions_expires_idx ON TABLE uia_sessions COLUMNS expires_at;
