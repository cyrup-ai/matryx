-- =====================================================
-- Migration: 111
-- Table: notification_settings
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/notification.rs
-- =====================================================

DEFINE TABLE notification_settings SCHEMAFULL
    PERMISSIONS
        FOR select, create, update WHERE $auth.user_id = user_id
        FOR delete WHERE $auth.user_id = user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE notification_settings TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD enabled ON TABLE notification_settings TYPE bool DEFAULT true;
DEFINE FIELD rules ON TABLE notification_settings TYPE object DEFAULT {};

DEFINE INDEX ns_user_idx ON TABLE notification_settings COLUMNS user_id UNIQUE;
