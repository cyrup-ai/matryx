-- =====================================================
-- Migration: 094
-- Table: edu
-- Entity: packages/entity/src/types/edu.rs
-- Repositories: packages/surrealdb/src/repository/edu.rs
-- =====================================================

-- EDU (Ephemeral Data Unit) table for Matrix federation ephemeral events
-- Spec: spec/server/07-edus.md
DEFINE TABLE edu SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.server_name != NONE OR $auth.user_id != NONE
        FOR create WHERE $auth.server_name != NONE
        FOR update, delete WHERE $auth.admin = true;

-- EDU type (m.typing, m.presence, m.receipt, m.device_list_update)
DEFINE FIELD edu_type ON TABLE edu TYPE string 
    ASSERT string::is::not::empty($value);

-- Origin server name (federation sender)
DEFINE FIELD origin ON TABLE edu TYPE option<string>;

-- Destination server name (federation receiver)
DEFINE FIELD destination ON TABLE edu TYPE option<string>;

-- Ephemeral event data
DEFINE FIELD ephemeral_event ON TABLE edu TYPE object;
DEFINE FIELD ephemeral_event.content ON TABLE edu TYPE object DEFAULT {};
DEFINE FIELD ephemeral_event.event_type ON TABLE edu TYPE string 
    ASSERT string::is::not::empty($value);
DEFINE FIELD ephemeral_event.room_id ON TABLE edu TYPE option<string>;
DEFINE FIELD ephemeral_event.sender ON TABLE edu TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

-- Non-persistent flag (EDUs are ephemeral, typically true)
DEFINE FIELD non_persistent ON TABLE edu TYPE bool DEFAULT true;

-- Timestamps for ordering and cleanup
DEFINE FIELD created_at ON TABLE edu TYPE datetime DEFAULT time::now();

-- Indexes for repository queries
-- Index for get_by_type() - most common query
DEFINE INDEX edu_type_idx ON TABLE edu COLUMNS edu_type;

-- Index for get_by_origin() - federation queries
DEFINE INDEX edu_origin_idx ON TABLE edu COLUMNS origin;

-- Index for get_by_destination() - federation queries
DEFINE INDEX edu_destination_idx ON TABLE edu COLUMNS destination;

-- Index for time-based ordering and cleanup
DEFINE INDEX edu_created_at_idx ON TABLE edu COLUMNS created_at;

-- Compound index for event type queries with time ordering
DEFINE INDEX edu_type_created_idx ON TABLE edu COLUMNS edu_type, created_at;

-- Index for device list stream queries (event_type + sender)
DEFINE INDEX edu_ephemeral_type_sender_idx ON TABLE edu 
    COLUMNS ephemeral_event.event_type, ephemeral_event.sender;
