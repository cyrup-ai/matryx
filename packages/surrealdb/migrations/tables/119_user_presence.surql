-- =====================================================
-- Migration: 119
-- Table: user_presence
-- Entity: Current user presence state
-- Repositories: presence.rs
-- =====================================================

-- Current user presence state for LiveQuery and queries
DEFINE TABLE user_presence SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE user_presence TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD presence ON TABLE user_presence TYPE string 
    ASSERT $value IN ['online', 'offline', 'unavailable'];

DEFINE FIELD last_active_ago ON TABLE user_presence TYPE option<int>;

DEFINE FIELD status_msg ON TABLE user_presence TYPE option<string>;

DEFINE FIELD currently_active ON TABLE user_presence TYPE bool DEFAULT false;

DEFINE FIELD updated_at ON TABLE user_presence TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns
DEFINE INDEX user_presence_user_idx ON TABLE user_presence COLUMNS user_id UNIQUE;
DEFINE INDEX user_presence_status_idx ON TABLE user_presence COLUMNS presence;
