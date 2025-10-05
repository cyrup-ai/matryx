-- =====================================================
-- Migration: 128
-- Table: event_reports
-- Entity: EventReport (repository struct line 52 of event.rs)
-- Repositories: event.rs
-- =====================================================

-- Event abuse reporting for content moderation
DEFINE TABLE event_reports SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true
        FOR create WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE event_reports TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD event_id ON TABLE event_reports TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '$') 
    AND string::contains($value, ':');

DEFINE FIELD room_id ON TABLE event_reports TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '!') 
    AND string::contains($value, ':');

DEFINE FIELD reporter_id ON TABLE event_reports TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD reason ON TABLE event_reports TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD score ON TABLE event_reports TYPE option<int>;

DEFINE FIELD created_at ON TABLE event_reports TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (lines 1105, 1150 of event.rs)
DEFINE INDEX event_reports_id_idx ON TABLE event_reports COLUMNS id UNIQUE;
DEFINE INDEX event_reports_event_reporter_idx ON TABLE event_reports COLUMNS event_id, reporter_id UNIQUE;
DEFINE INDEX event_reports_event_created_idx ON TABLE event_reports COLUMNS event_id, created_at;
