-- =====================================================
-- Migration: 029
-- Table: matrix_sync_device_list
-- Entity: packages/entity/src/types/matrix_sync_device_list.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Matrix device list updates for E2E encryption (matches Matrix spec)
DEFINE TABLE matrix_sync_device_list SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD user_id ON TABLE matrix_sync_device_list TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD changed_user_id ON TABLE matrix_sync_device_list TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_device_list TYPE int ASSERT $value > 0;
DEFINE FIELD device_change_type ON TABLE matrix_sync_device_list TYPE string ASSERT $value IN ['changed', 'left'];
DEFINE FIELD created_at ON TABLE matrix_sync_device_list TYPE datetime DEFAULT time::now();
