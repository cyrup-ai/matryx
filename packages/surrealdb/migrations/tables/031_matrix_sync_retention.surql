-- =====================================================
-- Migration: 031
-- Table: matrix_sync_retention
-- Entity: packages/entity/src/types/matrix_sync_retention.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Matrix sync retention configuration
DEFINE TABLE matrix_sync_retention SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.admin = true;

DEFINE FIELD table_name ON TABLE matrix_sync_retention TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD retention_days ON TABLE matrix_sync_retention TYPE int ASSERT $value > 0;
DEFINE FIELD cleanup_batch_size ON TABLE matrix_sync_retention TYPE int DEFAULT 1000;
DEFINE FIELD last_cleanup ON TABLE matrix_sync_retention TYPE datetime DEFAULT time::now();
