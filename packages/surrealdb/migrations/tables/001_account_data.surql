-- =====================================================
-- Migration: 001
-- Table: account_data
-- Entity: packages/entity/src/types/account_data.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Account data table for client configuration
DEFINE TABLE account_data SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.user_id = user_id OR $auth.is_admin = true
        FOR create, update, delete WHERE $auth.user_id = user_id OR $auth.is_admin = true;

DEFINE FIELD id ON TABLE account_data TYPE string ASSERT $value != "";
DEFINE FIELD user_id ON TABLE account_data TYPE string ASSERT $value != "";
DEFINE FIELD room_id ON TABLE account_data TYPE option<string>;
DEFINE FIELD data_type ON TABLE account_data TYPE string ASSERT $value != "";
DEFINE FIELD content ON TABLE account_data TYPE object;
DEFINE FIELD created_at ON TABLE account_data TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE account_data TYPE datetime DEFAULT time::now();

DEFINE INDEX account_data_user_lookup ON TABLE account_data COLUMNS user_id, room_id, data_type UNIQUE;
DEFINE INDEX account_data_user_idx ON TABLE account_data COLUMNS user_id;
DEFINE INDEX account_data_user_room_idx ON TABLE account_data COLUMNS user_id, room_id;
