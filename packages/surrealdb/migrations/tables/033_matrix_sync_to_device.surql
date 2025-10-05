-- =====================================================
-- Migration: 033
-- Table: matrix_sync_to_device
-- Entity: packages/entity/src/types/matrix_sync_to_device.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Matrix to_device messages for E2E encryption (matches Matrix spec)
DEFINE TABLE matrix_sync_to_device SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD user_id ON TABLE matrix_sync_to_device TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE matrix_sync_to_device TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD message_type ON TABLE matrix_sync_to_device TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_to_device TYPE int ASSERT $value > 0;
DEFINE FIELD content ON TABLE matrix_sync_to_device TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE matrix_sync_to_device TYPE datetime DEFAULT time::now();
