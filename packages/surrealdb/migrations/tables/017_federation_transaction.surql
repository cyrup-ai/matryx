-- =====================================================
-- Migration: 017
-- Table: federation_transaction
-- Entity: packages/entity/src/types/federation_transaction.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- FederationTransaction table - matches FederationTransaction entity exactly
DEFINE TABLE federation_transaction SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD origin ON TABLE federation_transaction TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server_ts ON TABLE federation_transaction TYPE int ASSERT $value > 0;
DEFINE FIELD pdus ON TABLE federation_transaction TYPE array<object> DEFAULT [];
DEFINE FIELD edus ON TABLE federation_transaction TYPE option<array<object>>;
DEFINE FIELD transaction_id ON TABLE federation_transaction TYPE string ASSERT string::is::not::empty($value);
