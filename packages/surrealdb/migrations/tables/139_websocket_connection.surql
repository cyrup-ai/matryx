-- =====================================================
-- Migration: 139
-- Table: websocket_connection
-- Entity: WebSocketConnection (websocket.rs line 8)
-- Repositories: websocket.rs
-- =====================================================

-- Active WebSocket connection tracking
DEFINE TABLE websocket_connection SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE $auth.user_id != NONE
        FOR update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD connection_id ON TABLE websocket_connection TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD user_id ON TABLE websocket_connection TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD device_id ON TABLE websocket_connection TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD created_at ON TABLE websocket_connection TYPE datetime DEFAULT time::now();

DEFINE FIELD last_seen ON TABLE websocket_connection TYPE datetime DEFAULT time::now();

DEFINE FIELD ip_address ON TABLE websocket_connection TYPE option<string>;

DEFINE FIELD user_agent ON TABLE websocket_connection TYPE option<string>;

-- Indexes based on repository query patterns (websocket.rs line 71)
DEFINE INDEX websocket_connection_connection_id_idx ON TABLE websocket_connection COLUMNS connection_id UNIQUE;
DEFINE INDEX websocket_connection_user_idx ON TABLE websocket_connection COLUMNS user_id;
DEFINE INDEX websocket_connection_last_seen_idx ON TABLE websocket_connection COLUMNS last_seen;
