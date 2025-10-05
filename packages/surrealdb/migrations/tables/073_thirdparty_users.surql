-- =====================================================
-- Migration: 073
-- Table: thirdparty_users
-- Entity: packages/entity/src/types/thirdparty_users.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Third-party users
DEFINE TABLE thirdparty_users SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD protocol ON TABLE thirdparty_users TYPE string;
DEFINE FIELD userid ON TABLE thirdparty_users TYPE string;
DEFINE FIELD fields ON TABLE thirdparty_users TYPE object;
DEFINE FIELD created_at ON TABLE thirdparty_users TYPE datetime DEFAULT time::now();
