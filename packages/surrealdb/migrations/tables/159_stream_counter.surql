-- =====================================================
-- Migration: 159
-- Table: stream_counter
-- Purpose: Atomic stream ID counter for Matrix sync
-- Related: packages/surrealdb/src/repository/sync.rs
-- =====================================================

-- Stream counter table - Single record for atomic increments
DEFINE TABLE stream_counter SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update WHERE $auth.user_id != NONE;

DEFINE FIELD position ON TABLE stream_counter TYPE int DEFAULT 0;
DEFINE FIELD updated_at ON TABLE stream_counter TYPE datetime DEFAULT time::now();

-- Initialize the global counter at position 0
CREATE stream_counter:global SET position = 0, updated_at = time::now();
