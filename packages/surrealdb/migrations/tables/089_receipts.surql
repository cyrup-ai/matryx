-- =====================================================
-- Migration: 089
-- Table: receipts
-- Entity: packages/entity/src/types/room_receipts.rs
-- Repositories: receipt.rs
-- =====================================================

-- Track read receipts (m.read, m.read.private)
DEFINE TABLE receipts SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE receipts TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '!') 
    AND string::contains($value, ':');

DEFINE FIELD user_id ON TABLE receipts TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD event_id ON TABLE receipts TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '$') 
    AND string::contains($value, ':');

DEFINE FIELD receipt_type ON TABLE receipts TYPE string 
    ASSERT $value IN ['m.read', 'm.read.private'];

DEFINE FIELD thread_id ON TABLE receipts TYPE option<string>;
DEFINE FIELD timestamp ON TABLE receipts TYPE int ASSERT $value > 0;
DEFINE FIELD is_private ON TABLE receipts TYPE bool DEFAULT false;
DEFINE FIELD server_name ON TABLE receipts TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD received_at ON TABLE receipts TYPE datetime DEFAULT time::now();

DEFINE INDEX receipts_room_user_type_idx ON TABLE receipts 
    COLUMNS room_id, user_id, receipt_type UNIQUE;
DEFINE INDEX receipts_room_idx ON TABLE receipts 
    COLUMNS room_id;
DEFINE INDEX receipts_user_idx ON TABLE receipts 
    COLUMNS user_id;
DEFINE INDEX receipts_timestamp_idx ON TABLE receipts 
    COLUMNS timestamp;
