-- =====================================================
-- Migration: 087
-- Table: users
-- Entity: packages/entity/src/types/users.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Users table - Alternative user table name (queried by user.rs)
DEFINE TABLE users SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.admin = true
        FOR update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE users TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD display_name ON TABLE users TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE users TYPE option<string>;
DEFINE FIELD password_hash ON TABLE users TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE users TYPE datetime DEFAULT time::now();
DEFINE FIELD is_active ON TABLE users TYPE bool DEFAULT true;
DEFINE FIELD is_admin ON TABLE users TYPE bool DEFAULT false;
