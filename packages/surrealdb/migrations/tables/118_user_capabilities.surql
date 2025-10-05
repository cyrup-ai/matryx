-- =====================================================
-- Migration: 118
-- Table: user_capabilities
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/capabilities.rs, packages/surrealdb/src/repository/client_api_service.rs
-- =====================================================

DEFINE TABLE user_capabilities SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id = user_id
        FOR create, update WHERE $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE user_capabilities TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD can_create_rooms ON TABLE user_capabilities TYPE bool DEFAULT true;
DEFINE FIELD can_invite_users ON TABLE user_capabilities TYPE bool DEFAULT true;
DEFINE FIELD can_join_public_rooms ON TABLE user_capabilities TYPE bool DEFAULT true;
DEFINE FIELD can_change_password ON TABLE user_capabilities TYPE bool DEFAULT true;
DEFINE FIELD can_set_displayname ON TABLE user_capabilities TYPE bool DEFAULT true;
DEFINE FIELD can_set_avatar_url ON TABLE user_capabilities TYPE bool DEFAULT true;

DEFINE INDEX uc_user_idx ON TABLE user_capabilities COLUMNS user_id UNIQUE;
