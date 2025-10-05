-- =====================================================
-- Migration: 077
-- Table: user
-- Entity: packages/entity/src/types/user.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- User table - Core Matrix user entity (matches User entity from packages/entity/src/types/user.rs)
DEFINE TABLE user SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.admin = true
        FOR update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE user TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD display_name ON TABLE user TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE user TYPE option<string>;
DEFINE FIELD password_hash ON TABLE user TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE user TYPE datetime DEFAULT time::now();
DEFINE FIELD last_seen ON TABLE user TYPE option<datetime>;
DEFINE FIELD is_active ON TABLE user TYPE bool DEFAULT true;
DEFINE FIELD is_admin ON TABLE user TYPE bool DEFAULT false;
DEFINE FIELD account_data ON TABLE user TYPE option<object>;

DEFINE INDEX user_id_idx ON TABLE user COLUMNS user_id UNIQUE;
DEFINE INDEX user_active_idx ON TABLE user COLUMNS is_active;
DEFINE INDEX user_admin_idx ON TABLE user COLUMNS is_admin;
