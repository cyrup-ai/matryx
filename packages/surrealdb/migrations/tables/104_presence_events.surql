-- =====================================================
-- Migration: 104
-- Table: presence_events
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/presence.rs, packages/surrealdb/src/repository/sync.rs
-- =====================================================

DEFINE TABLE presence_events SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update WHERE $auth.user_id = user_id
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE presence_events TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD presence ON TABLE presence_events TYPE string ASSERT $value IN ['online', 'offline', 'unavailable'];
DEFINE FIELD last_active_ago ON TABLE presence_events TYPE option<int>;
DEFINE FIELD status_msg ON TABLE presence_events TYPE option<string>;
DEFINE FIELD updated_at ON TABLE presence_events TYPE datetime DEFAULT time::now();

DEFINE INDEX pe_user_idx ON TABLE presence_events COLUMNS user_id;
DEFINE INDEX pe_updated_idx ON TABLE presence_events COLUMNS updated_at;
DEFINE INDEX pe_user_updated_idx ON TABLE presence_events COLUMNS user_id, updated_at;
