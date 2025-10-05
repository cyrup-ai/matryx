-- =====================================================
-- Migration: 045
-- Table: pushers
-- Entity: packages/entity/src/types/pushers.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Pushers (devices registered for notifications)
DEFINE TABLE pushers SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE user_id = $auth.user_id
        FOR create, update, delete WHERE user_id = $auth.user_id;

DEFINE FIELD id ON TABLE pushers TYPE string;
DEFINE FIELD user_id ON TABLE pushers TYPE string;
DEFINE FIELD pusher_id ON TABLE pushers TYPE string;
DEFINE FIELD kind ON TABLE pushers TYPE string; -- http, email, etc.
DEFINE FIELD app_id ON TABLE pushers TYPE string;
DEFINE FIELD app_display_name ON TABLE pushers TYPE string;
DEFINE FIELD device_display_name ON TABLE pushers TYPE string;
DEFINE FIELD profile_tag ON TABLE pushers TYPE option<string>;
DEFINE FIELD lang ON TABLE pushers TYPE string;
DEFINE FIELD data ON TABLE pushers TYPE object; -- gateway URL, format, etc.
DEFINE FIELD created_at ON TABLE pushers TYPE datetime DEFAULT time::now();
