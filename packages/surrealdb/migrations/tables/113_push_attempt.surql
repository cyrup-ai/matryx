-- =====================================================
-- Migration: 113
-- Table: push_attempt
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/push_gateway.rs
-- =====================================================

DEFINE TABLE push_attempt SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD pusher_key ON TABLE push_attempt TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD notification_id ON TABLE push_attempt TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD success ON TABLE push_attempt TYPE bool DEFAULT false;
DEFINE FIELD error_message ON TABLE push_attempt TYPE option<string>;
DEFINE FIELD attempted_at ON TABLE push_attempt TYPE datetime DEFAULT time::now();

DEFINE INDEX pa_pusher_idx ON TABLE push_attempt COLUMNS pusher_key;
DEFINE INDEX pa_attempted_idx ON TABLE push_attempt COLUMNS attempted_at;
