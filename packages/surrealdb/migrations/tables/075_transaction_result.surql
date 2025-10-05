-- =====================================================
-- Migration: 075
-- Table: transaction_result
-- Entity: packages/entity/src/types/transaction_result.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- TransactionResult table - matches TransactionResult entity exactly
DEFINE TABLE transaction_result SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.server_name != NONE  -- Federation table
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD transaction_id ON TABLE transaction_result TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin ON TABLE transaction_result TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server_ts ON TABLE transaction_result TYPE int ASSERT $value > 0;
DEFINE FIELD data ON TABLE transaction_result TYPE option<object>;
