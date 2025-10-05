-- =====================================================
-- Migration: 039
-- Table: pdu
-- Entity: packages/entity/src/types/pdu.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- PDU table - Matrix federation events (server-to-server, different auth than client-server)
DEFINE TABLE pdu SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.server_name != NONE  -- Federation authentication
        FOR create WHERE $auth.server_name != NONE  -- Only federated servers can create PDUs
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD event_id ON TABLE pdu TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD room_id ON TABLE pdu TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD sender ON TABLE pdu TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD event_type ON TABLE pdu TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD content ON TABLE pdu TYPE object DEFAULT {};
DEFINE FIELD state_key ON TABLE pdu TYPE option<string>;
DEFINE FIELD origin_server_ts ON TABLE pdu TYPE int ASSERT $value > 0;
DEFINE FIELD prev_events ON TABLE pdu TYPE array<string> DEFAULT [];
DEFINE FIELD auth_events ON TABLE pdu TYPE array<string> DEFAULT [];
DEFINE FIELD depth ON TABLE pdu TYPE int DEFAULT 0;
DEFINE FIELD signatures ON TABLE pdu TYPE object DEFAULT {};
DEFINE FIELD hashes ON TABLE pdu TYPE object DEFAULT {};
DEFINE FIELD unsigned ON TABLE pdu TYPE option<object>;

DEFINE INDEX pdu_event_id_idx ON TABLE pdu COLUMNS event_id UNIQUE;
DEFINE INDEX pdu_room_ts_idx ON TABLE pdu COLUMNS room_id, origin_server_ts;
DEFINE INDEX pdu_sender_idx ON TABLE pdu COLUMNS sender;
DEFINE INDEX pdu_type_idx ON TABLE pdu COLUMNS event_type;
DEFINE INDEX pdu_state_idx ON TABLE pdu COLUMNS room_id, event_type, state_key;
