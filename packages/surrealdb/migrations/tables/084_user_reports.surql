-- =====================================================
-- Migration: 084
-- Table: user_reports
-- Entity: packages/entity/src/types/user_reports.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- User reports table - Content moderation reports (queried by reports.rs)
DEFINE TABLE user_reports SCHEMAFULL
    PERMISSIONS
        FOR select WHERE reporter_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE reporter_id = $auth.user_id
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD report_id ON TABLE user_reports TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD reporter_id ON TABLE user_reports TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD reported_user_id ON TABLE user_reports TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE user_reports TYPE option<string>;
DEFINE FIELD event_id ON TABLE user_reports TYPE option<string>;
DEFINE FIELD reason ON TABLE user_reports TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD score ON TABLE user_reports TYPE option<int>;
DEFINE FIELD status ON TABLE user_reports TYPE string DEFAULT 'pending' ASSERT $value IN ['pending', 'investigating', 'escalated', 'resolved', 'dismissed'];
DEFINE FIELD created_at ON TABLE user_reports TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE user_reports TYPE datetime DEFAULT time::now();
