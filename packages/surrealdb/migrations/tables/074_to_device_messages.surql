-- =====================================================
-- Migration: 074
-- Table: to_device_messages
-- Entity: packages/entity/src/types/to_device_messages.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- To-device messages table - Direct device-to-device messages (queried by to_device.rs)
DEFINE TABLE to_device_messages SCHEMAFULL
    PERMISSIONS
        FOR select WHERE recipient_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE recipient_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD message_id ON TABLE to_device_messages TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sender_id ON TABLE to_device_messages TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD recipient_id ON TABLE to_device_messages TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE to_device_messages TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD message_type ON TABLE to_device_messages TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD content ON TABLE to_device_messages TYPE object DEFAULT {};
DEFINE FIELD is_delivered ON TABLE to_device_messages TYPE bool DEFAULT false;
DEFINE FIELD delivered_at ON TABLE to_device_messages TYPE option<datetime>;
DEFINE FIELD created_at ON TABLE to_device_messages TYPE datetime DEFAULT time::now();
