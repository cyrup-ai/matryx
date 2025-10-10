-- =====================================================
-- Migration: 076
-- Table: typing_notification
-- Entity: packages/entity/src/types/typing_notification.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- TypingNotification table - matches TypingNotification entity exactly
DEFINE TABLE typing_notification SCHEMAFULL
    PERMISSIONS
        FOR select WHERE room_id IN (SELECT room_id FROM membership_event_content WHERE user_id = $auth.user_id AND membership = 'join')
        FOR create WHERE user_id = $auth.user_id
        FOR update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD room_id ON TABLE typing_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD typing ON TABLE typing_notification TYPE bool DEFAULT false;
DEFINE FIELD user_id ON TABLE typing_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD server_name ON TABLE typing_notification TYPE string;
DEFINE FIELD started_at ON TABLE typing_notification TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE typing_notification TYPE datetime;
DEFINE FIELD updated_at ON TABLE typing_notification TYPE datetime DEFAULT time::now();

-- Indexes for efficient cleanup and queries
DEFINE INDEX idx_typing_expires ON TABLE typing_notification FIELDS expires_at;
DEFINE INDEX idx_typing_room ON TABLE typing_notification FIELDS room_id, expires_at;
