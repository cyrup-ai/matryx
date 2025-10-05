-- =====================================================
-- Migration: 047
-- Table: room
-- Entity: packages/entity/src/types/room.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room table - Core Matrix room entity (matches Room entity from packages/entity/src/types/room.rs)
DEFINE TABLE room SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update WHERE creator = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE room TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD room_version ON TABLE room TYPE string DEFAULT '10';
DEFINE FIELD creator ON TABLE room TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD created_at ON TABLE room TYPE datetime DEFAULT time::now();
DEFINE FIELD name ON TABLE room TYPE option<string>;
DEFINE FIELD topic ON TABLE room TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE room TYPE option<string>;
DEFINE FIELD canonical_alias ON TABLE room TYPE option<string>;
DEFINE FIELD alt_aliases ON TABLE room TYPE option<array<string>>;
DEFINE FIELD join_rule ON TABLE room TYPE option<string>;
DEFINE FIELD history_visibility ON TABLE room TYPE option<string>;
DEFINE FIELD guest_access ON TABLE room TYPE option<string>;
DEFINE FIELD power_levels ON TABLE room TYPE option<object>;
DEFINE FIELD encryption ON TABLE room TYPE option<object>;
DEFINE FIELD federate ON TABLE room TYPE option<bool> DEFAULT true;
DEFINE FIELD room_type ON TABLE room TYPE option<string>;
DEFINE FIELD is_public ON TABLE room TYPE option<bool> DEFAULT false;
DEFINE FIELD is_direct ON TABLE room TYPE option<bool> DEFAULT false;
DEFINE FIELD join_rules ON TABLE room TYPE option<string>;
DEFINE FIELD tombstone ON TABLE room TYPE option<object>;
DEFINE FIELD predecessor ON TABLE room TYPE option<object>;
DEFINE FIELD state_events_count ON TABLE room TYPE option<int> DEFAULT 0;
DEFINE FIELD updated_at ON TABLE room TYPE option<datetime>;
DEFINE FIELD visibility ON TABLE room TYPE string DEFAULT 'private';

DEFINE INDEX room_id_idx ON TABLE room COLUMNS room_id UNIQUE;
DEFINE INDEX room_creator_idx ON TABLE room COLUMNS creator;
DEFINE INDEX room_visibility_idx ON TABLE room COLUMNS visibility;
DEFINE INDEX room_is_public_idx ON TABLE room COLUMNS is_public;
DEFINE INDEX room_type_idx ON TABLE room COLUMNS room_type;
