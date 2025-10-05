-- =====================================================
-- Migration: 081
-- Table: user_blocks
-- Entity: packages/entity/src/types/user_blocks.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- User blocks table - User blocking functionality (queried by to_device.rs)
DEFINE TABLE user_blocks SCHEMAFULL
    PERMISSIONS
        FOR select WHERE blocker_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE blocker_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD id ON TABLE user_blocks TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD blocker_id ON TABLE user_blocks TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD blocked_id ON TABLE user_blocks TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE user_blocks TYPE datetime DEFAULT time::now();
