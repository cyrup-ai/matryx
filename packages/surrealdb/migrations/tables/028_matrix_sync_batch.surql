-- =====================================================
-- Migration: 028
-- Table: matrix_sync_batch
-- Entity: packages/entity/src/types/matrix_sync_batch.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Matrix sync batch tracking for /sync endpoint batch tokens
DEFINE TABLE matrix_sync_batch SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE matrix_sync_batch TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD batch_token ON TABLE matrix_sync_batch TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_batch TYPE int DEFAULT 0;
DEFINE FIELD filter_id ON TABLE matrix_sync_batch TYPE option<string>; -- Optional filter applied to sync
DEFINE FIELD timeout ON TABLE matrix_sync_batch TYPE option<int>; -- Timeout for this sync request  
DEFINE FIELD full_state ON TABLE matrix_sync_batch TYPE bool DEFAULT false; -- Whether to include full state
DEFINE FIELD created_at ON TABLE matrix_sync_batch TYPE datetime DEFAULT time::now();
