-- =====================================================
-- Migration: 014
-- Table: event_template
-- Entity: packages/entity/src/types/event_template.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- EventTemplate table - matches EventTemplate entity exactly
DEFINE TABLE event_template SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update, delete WHERE sender = $auth.user_id OR $auth.admin = true;

DEFINE FIELD event_id ON TABLE event_template TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD sender ON TABLE event_template TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD origin_server_ts ON TABLE event_template TYPE int ASSERT $value > 0;
DEFINE FIELD event_type ON TABLE event_template TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE event_template TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD state_key ON TABLE event_template TYPE option<string>;
DEFINE FIELD content ON TABLE event_template TYPE object DEFAULT {};
DEFINE FIELD unsigned ON TABLE event_template TYPE option<object>;
