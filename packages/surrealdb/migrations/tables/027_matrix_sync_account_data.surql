-- =====================================================
-- Migration: 027
-- Table: matrix_sync_account_data
-- Entity: packages/entity/src/types/matrix_sync_account_data.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Matrix sync account data for /sync account_data section
DEFINE TABLE matrix_sync_account_data SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE matrix_sync_account_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE matrix_sync_account_data TYPE option<string>; -- null for global account data
DEFINE FIELD data_type ON TABLE matrix_sync_account_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_account_data TYPE int ASSERT $value > 0;
DEFINE FIELD content ON TABLE matrix_sync_account_data TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE matrix_sync_account_data TYPE datetime DEFAULT time::now();
