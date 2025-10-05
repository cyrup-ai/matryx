-- =====================================================
-- Migration: 012
-- Table: event_reactions
-- Entity: packages/entity/src/types/event_reactions.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Reaction aggregations
DEFINE TABLE event_reactions SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.room_id AND membership = 'join')
        FOR create, update, delete WHERE $auth.user_id = user_id;

DEFINE FIELD event_id ON TABLE event_reactions TYPE string ASSERT $value != "";
DEFINE FIELD room_id ON TABLE event_reactions TYPE string ASSERT $value != "";
DEFINE FIELD reaction_key ON TABLE event_reactions TYPE string ASSERT $value != "";
DEFINE FIELD user_id ON TABLE event_reactions TYPE string ASSERT $value != "";
DEFINE FIELD created_at ON TABLE event_reactions TYPE datetime DEFAULT time::now();
