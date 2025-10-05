-- =====================================================
-- Migration: 086
-- Table: user_threepids
-- Entity: packages/entity/src/types/user_threepids.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Third-party identifiers table
DEFINE TABLE user_threepids SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.user_id = user_id OR $auth.is_admin = true
        FOR create, update, delete WHERE $auth.user_id = user_id OR $auth.is_admin = true;

DEFINE FIELD id ON TABLE user_threepids TYPE string ASSERT $value != "";
DEFINE FIELD user_id ON TABLE user_threepids TYPE string ASSERT $value != "";
DEFINE FIELD medium ON TABLE user_threepids TYPE string ASSERT $value IN ['email', 'msisdn'];
DEFINE FIELD address ON TABLE user_threepids TYPE string ASSERT $value != "";
DEFINE FIELD validated_at ON TABLE user_threepids TYPE option<datetime>;
DEFINE FIELD added_at ON TABLE user_threepids TYPE datetime DEFAULT time::now();
