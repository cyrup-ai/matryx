-- =====================================================
-- Migration: 078
-- Table: user_3pids
-- Entity: packages/entity/src/types/user_3pids.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- User 3PIDs table for storing verified third-party identifiers
DEFINE TABLE user_3pids SCHEMAFULL
    PERMISSIONS
        FOR select, create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE user_3pids TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD medium ON TABLE user_3pids TYPE string ASSERT $value IN ["email", "msisdn"];
DEFINE FIELD address ON TABLE user_3pids TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD validated_at ON TABLE user_3pids TYPE int ASSERT $value > 0;
DEFINE FIELD added_at ON TABLE user_3pids TYPE int ASSERT $value > 0;
