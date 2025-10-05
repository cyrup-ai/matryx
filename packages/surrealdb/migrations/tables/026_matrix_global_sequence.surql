-- =====================================================
-- Migration: 026
-- Table: matrix_global_sequence
-- Entity: packages/entity/src/types/matrix_global_sequence.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Global sequence number for Matrix sync ordering
DEFINE TABLE matrix_global_sequence SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true
        FOR create, update, delete WHERE $auth.admin = true;
DEFINE FIELD current_sequence ON TABLE matrix_global_sequence TYPE int DEFAULT 0;
