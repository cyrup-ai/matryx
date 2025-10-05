-- =====================================================
-- Migration: 080
-- Table: user_account_data
-- Entity: packages/entity/src/types/user_account_data.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- UserAccountData table - matches Matrix account data storage specification
DEFINE TABLE user_account_data SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE user_account_data TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD room_id ON TABLE user_account_data TYPE option<string> 
    ASSERT ($value IS NONE) OR (string::starts_with($value, '!') AND string::contains($value, ':')); -- null for global account data or valid room ID
DEFINE FIELD type ON TABLE user_account_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD content ON TABLE user_account_data TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE user_account_data TYPE datetime DEFAULT time::now();
