-- =====================================================
-- Migration: 030
-- Table: matrix_sync_presence
-- Entity: packages/entity/src/types/matrix_sync_presence.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Matrix sync presence updates for /sync presence section
DEFINE TABLE matrix_sync_presence SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE matrix_sync_presence TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD target_user_id ON TABLE matrix_sync_presence TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_presence TYPE int ASSERT $value > 0;
DEFINE FIELD presence_content ON TABLE matrix_sync_presence TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE matrix_sync_presence TYPE datetime DEFAULT time::now();
