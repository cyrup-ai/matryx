-- =====================================================
-- Migration: 112
-- Table: push_notification
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/push_notification.rs
-- =====================================================

DEFINE TABLE push_notification SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD notification_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD notification_data ON TABLE push_notification TYPE object;
DEFINE FIELD notification_data.user_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD notification_data.event_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD notification_data.room_id ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD notification_data.pusher_key ON TABLE push_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD notification_data.status ON TABLE push_notification TYPE string DEFAULT 'Pending';
DEFINE FIELD notification_data.created_at ON TABLE push_notification TYPE datetime DEFAULT time::now();
DEFINE FIELD attempts ON TABLE push_notification TYPE int DEFAULT 0;

DEFINE INDEX pn_user_idx ON TABLE push_notification COLUMNS notification_data.user_id;
DEFINE INDEX pn_status_idx ON TABLE push_notification COLUMNS notification_data.status;
