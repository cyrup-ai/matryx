-- =====================================================
-- Migration: 011
-- Table: event
-- Entity: packages/entity/src/types/event.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Event table - Matrix spec compliant base Event (spec/client/04_security_encryption.md:92-96)
DEFINE TABLE event SCHEMAFULL
    PERMISSIONS
        FOR select, create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD event_id ON TABLE event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD sender ON TABLE event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD origin_server_ts ON TABLE event TYPE int ASSERT $value > 0;
DEFINE FIELD event_type ON TABLE event TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD content ON TABLE event TYPE object DEFAULT {};
DEFINE FIELD state_key ON TABLE event TYPE option<string>;
DEFINE FIELD unsigned ON TABLE event TYPE option<object>;

DEFINE INDEX event_id_idx ON TABLE event COLUMNS event_id UNIQUE;
DEFINE INDEX event_room_ts_idx ON TABLE event COLUMNS room_id, origin_server_ts;
DEFINE INDEX event_sender_idx ON TABLE event COLUMNS sender;
