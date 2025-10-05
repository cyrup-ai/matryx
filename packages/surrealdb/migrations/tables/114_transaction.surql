-- =====================================================
-- Migration: 114
-- Table: transaction
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/transaction.rs, packages/surrealdb/src/repository/event.rs
-- =====================================================

DEFINE TABLE transaction SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD txn_id ON TABLE transaction TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin ON TABLE transaction TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD destination ON TABLE transaction TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD pdus ON TABLE transaction TYPE array DEFAULT [];
DEFINE FIELD edus ON TABLE transaction TYPE array DEFAULT [];
DEFINE FIELD created_at ON TABLE transaction TYPE datetime DEFAULT time::now();

DEFINE INDEX txn_id_idx ON TABLE transaction COLUMNS txn_id;
DEFINE INDEX txn_origin_dest_idx ON TABLE transaction COLUMNS origin, destination;
