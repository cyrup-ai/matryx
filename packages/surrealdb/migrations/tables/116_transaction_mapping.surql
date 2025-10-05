-- =====================================================
-- Migration: 116
-- Table: transaction_mapping
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/event.rs
-- =====================================================

DEFINE TABLE transaction_mapping SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id = user_id
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE transaction_mapping TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD txn_id ON TABLE transaction_mapping TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD event_id ON TABLE transaction_mapping TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE transaction_mapping TYPE datetime DEFAULT time::now();

DEFINE INDEX tm_user_txn_idx ON TABLE transaction_mapping COLUMNS user_id, txn_id;
DEFINE INDEX tm_event_idx ON TABLE transaction_mapping COLUMNS event_id;
