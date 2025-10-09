-- =====================================================
-- Migration: 105
-- Table: notifications
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/sync.rs
-- =====================================================

DEFINE TABLE notifications SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id = user_id
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.user_id = user_id OR $auth.admin = true;

DEFINE FIELD notification_id ON TABLE notifications TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE notifications TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD event_id ON TABLE notifications TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD room_id ON TABLE notifications TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD read ON TABLE notifications TYPE bool DEFAULT false;
DEFINE FIELD highlight ON TABLE notifications TYPE bool DEFAULT false;
DEFINE FIELD created_at ON TABLE notifications TYPE datetime DEFAULT time::now();

DEFINE INDEX notif_user_idx ON TABLE notifications COLUMNS user_id;
DEFINE INDEX notif_user_read_idx ON TABLE notifications COLUMNS user_id, read;
DEFINE INDEX notif_user_created_idx ON TABLE notifications COLUMNS user_id, created_at;
DEFINE INDEX notif_user_highlight_idx ON TABLE notifications COLUMNS user_id, highlight;
DEFINE INDEX notif_created_idx ON TABLE notifications COLUMNS created_at;
