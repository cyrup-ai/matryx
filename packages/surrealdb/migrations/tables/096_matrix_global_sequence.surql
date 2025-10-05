-- =====================================================
-- Migration: 096
-- Table: matrix_global_sequence
-- Purpose: Global sequence counter for sync event ordering
-- =====================================================

-- Global sequence counter for ordering all sync events
DEFINE TABLE matrix_global_sequence SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update WHERE $auth.admin = true
        FOR delete WHERE false;

DEFINE FIELD id ON TABLE matrix_global_sequence TYPE string DEFAULT 'global' ASSERT $value = 'global';
DEFINE FIELD sequence_number ON TABLE matrix_global_sequence TYPE int DEFAULT 0 ASSERT $value >= 0;
DEFINE FIELD updated_at ON TABLE matrix_global_sequence TYPE datetime DEFAULT time::now();

DEFINE INDEX matrix_global_sequence_id_idx ON TABLE matrix_global_sequence COLUMNS id UNIQUE;

-- Initialize the sequence counter
CREATE matrix_global_sequence:global SET
    id = 'global',
    sequence_number = 0,
    updated_at = time::now();
