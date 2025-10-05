-- =====================================================
-- Migration: 018
-- Table: federation_transactions
-- Entity: packages/entity/src/types/federation_transactions.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Federation transactions cache for deduplication and reliability
DEFINE TABLE federation_transactions SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.admin = true;

DEFINE FIELD transaction_key ON TABLE federation_transactions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server ON TABLE federation_transactions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD transaction_id ON TABLE federation_transactions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD result ON TABLE federation_transactions TYPE object DEFAULT {};
DEFINE FIELD status ON TABLE federation_transactions TYPE string DEFAULT 'processed' ASSERT $value IN ['processing', 'processed', 'failed'];
DEFINE FIELD created_at ON TABLE federation_transactions TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE federation_transactions TYPE datetime DEFAULT (time::now() + duration::hours(24));
