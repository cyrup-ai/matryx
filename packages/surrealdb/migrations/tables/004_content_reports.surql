-- =====================================================
-- Migration: 004
-- Table: content_reports
-- Entity: packages/entity/src/types/content_reports.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Content reports
DEFINE TABLE content_reports SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.is_admin = true OR $auth.user_id = reporter_user_id
        FOR create WHERE $auth.user_id = reporter_user_id
        FOR update, delete WHERE $auth.is_admin = true;

DEFINE FIELD id ON TABLE content_reports TYPE string ASSERT $value != "";
DEFINE FIELD reporter_user_id ON TABLE content_reports TYPE string ASSERT $value != "";
DEFINE FIELD reported_user_id ON TABLE content_reports TYPE option<string>;
DEFINE FIELD room_id ON TABLE content_reports TYPE option<string>;
DEFINE FIELD event_id ON TABLE content_reports TYPE option<string>;
DEFINE FIELD reason ON TABLE content_reports TYPE string ASSERT $value != "";
DEFINE FIELD score ON TABLE content_reports TYPE option<int>;
DEFINE FIELD status ON TABLE content_reports TYPE string DEFAULT 'pending';
DEFINE FIELD created_at ON TABLE content_reports TYPE datetime DEFAULT time::now();
DEFINE FIELD resolved_at ON TABLE content_reports TYPE option<datetime>;
