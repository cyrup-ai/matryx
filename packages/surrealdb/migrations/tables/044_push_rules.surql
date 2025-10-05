-- =====================================================
-- Migration: 044
-- Table: push_rules
-- Entity: packages/entity/src/types/push_rules.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Push rules storage
DEFINE TABLE push_rules SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE user_id = $auth.user_id
        FOR create, update, delete WHERE user_id = $auth.user_id;

DEFINE FIELD id ON TABLE push_rules TYPE string;
DEFINE FIELD user_id ON TABLE push_rules TYPE string;
DEFINE FIELD rule_id ON TABLE push_rules TYPE string;
DEFINE FIELD kind ON TABLE push_rules TYPE string; -- override, content, room, sender, underride
DEFINE FIELD priority ON TABLE push_rules TYPE int;
DEFINE FIELD conditions ON TABLE push_rules TYPE array<object>;
DEFINE FIELD actions ON TABLE push_rules TYPE array<object>;
DEFINE FIELD enabled ON TABLE push_rules TYPE bool DEFAULT true;
DEFINE FIELD default ON TABLE push_rules TYPE bool DEFAULT false;
DEFINE FIELD created_at ON TABLE push_rules TYPE datetime DEFAULT time::now();
