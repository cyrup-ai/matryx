-- =====================================================
-- Migration: 106
-- Table: room_account_data
-- Entity: N/A
-- Repositories: packages/surrealdb/src/repository/sync.rs, packages/surrealdb/src/repository/event.rs
-- =====================================================

DEFINE TABLE room_account_data SCHEMAFULL
    PERMISSIONS
        FOR select, create, update WHERE $auth.user_id = user_id
        FOR delete WHERE $auth.user_id = user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE room_account_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE room_account_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD data_type ON TABLE room_account_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD content ON TABLE room_account_data TYPE object DEFAULT {};
DEFINE FIELD updated_at ON TABLE room_account_data TYPE datetime DEFAULT time::now();

DEFINE INDEX rad_user_room_idx ON TABLE room_account_data COLUMNS user_id, room_id;
DEFINE INDEX rad_user_room_type_idx ON TABLE room_account_data COLUMNS user_id, room_id, data_type UNIQUE;
DEFINE INDEX rad_user_room_updated_idx ON TABLE room_account_data COLUMNS user_id, room_id, updated_at;
