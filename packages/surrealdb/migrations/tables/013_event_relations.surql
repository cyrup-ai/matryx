-- =====================================================
-- Migration: 013
-- Table: event_relations
-- Entity: packages/entity/src/types/event_relations.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Event relations table - Event relationships (replies, edits, reactions) (queried by event_replacement.rs)
DEFINE TABLE event_relations SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD event_id ON TABLE event_relations TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD relates_to_event_id ON TABLE event_relations TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD rel_type ON TABLE event_relations TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE event_relations TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE event_relations TYPE datetime DEFAULT time::now();
