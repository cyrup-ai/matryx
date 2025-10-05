-- =====================================================
-- Migration: 082
-- Table: user_presence_update
-- Entity: packages/entity/src/types/user_presence_update.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- UserPresenceUpdate table - matches UserPresenceUpdate entity exactly
DEFINE TABLE user_presence_update SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Presence is visible to users in shared rooms
        FOR create, update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE user_presence_update TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD presence ON TABLE user_presence_update TYPE string ASSERT $value IN ['online', 'offline', 'unavailable'];
DEFINE FIELD last_active_ago ON TABLE user_presence_update TYPE option<int>;
DEFINE FIELD status_msg ON TABLE user_presence_update TYPE option<string>;
DEFINE FIELD currently_active ON TABLE user_presence_update TYPE option<bool>;
