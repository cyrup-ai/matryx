-- =====================================================
-- Migration: 043
-- Table: push_rule
-- Entity: packages/entity/src/types/push_rule.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- PushRule table - Matrix push notification rules (matches PushRule entity exactly)
DEFINE TABLE push_rule SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.user_id != NONE;  -- Users manage their push rules

DEFINE FIELD rule_id ON TABLE push_rule TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD default ON TABLE push_rule TYPE bool DEFAULT false;
DEFINE FIELD enabled ON TABLE push_rule TYPE bool DEFAULT true;
DEFINE FIELD pattern ON TABLE push_rule TYPE option<string>;
DEFINE FIELD conditions ON TABLE push_rule TYPE option<array<object>>;
DEFINE FIELD actions ON TABLE push_rule TYPE array<object> DEFAULT [];
