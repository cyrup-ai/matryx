-- =====================================================
-- Migration: 115
-- Table: transaction_dedupe
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/transaction.rs
-- =====================================================

DEFINE TABLE transaction_dedupe SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id = user_id
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE transaction_dedupe TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD txn_id ON TABLE transaction_dedupe TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD endpoint ON TABLE transaction_dedupe TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD result ON TABLE transaction_dedupe TYPE object;
DEFINE FIELD created_at ON TABLE transaction_dedupe TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE transaction_dedupe TYPE datetime;

DEFINE INDEX td_user_txn_endpoint_idx ON TABLE transaction_dedupe COLUMNS user_id, txn_id, endpoint UNIQUE;
DEFINE INDEX td_expires_idx ON TABLE transaction_dedupe COLUMNS expires_at;
