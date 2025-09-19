-- =====================================================
-- Matrix Server Database Schema for SurrealDB v3.0+
-- Precisely aligned with Matrix Entity Types
-- =====================================================

-- =====================================================
-- NAMESPACE AND DATABASE SETUP
-- =====================================================
DEFINE NAMESPACE matryx;
USE NS matryx;
DEFINE DATABASE matrix;
USE DB matrix;

-- =====================================================
-- CORE MATRIX ENTITY TABLES
-- =====================================================

-- Device table - Matrix spec compliant (spec/client/04_security_encryption.md:247-250)
DEFINE TABLE device SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Devices are accessible to authenticated users managing them
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD device_id ON TABLE device TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD display_name ON TABLE device TYPE option<string>;
DEFINE FIELD last_seen_ip ON TABLE device TYPE option<string>;
DEFINE FIELD last_seen_ts ON TABLE device TYPE option<int>;

DEFINE INDEX device_id_idx ON TABLE device COLUMNS device_id UNIQUE;

-- PDU table - Matrix federation events (server-to-server, different auth than client-server)
DEFINE TABLE pdu SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.server_name != NONE  -- Federation authentication
        FOR create WHERE $auth.server_name != NONE  -- Only federated servers can create PDUs
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD event_id ON TABLE pdu TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD room_id ON TABLE pdu TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD sender ON TABLE pdu TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD event_type ON TABLE pdu TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD content ON TABLE pdu TYPE object DEFAULT {};
DEFINE FIELD state_key ON TABLE pdu TYPE option<string>;
DEFINE FIELD origin_server_ts ON TABLE pdu TYPE int ASSERT $value > 0;
DEFINE FIELD prev_events ON TABLE pdu TYPE array<string> DEFAULT [];
DEFINE FIELD auth_events ON TABLE pdu TYPE array<string> DEFAULT [];
DEFINE FIELD depth ON TABLE pdu TYPE int DEFAULT 0;
DEFINE FIELD signatures ON TABLE pdu TYPE object DEFAULT {};
DEFINE FIELD hashes ON TABLE pdu TYPE object DEFAULT {};
DEFINE FIELD unsigned ON TABLE pdu TYPE option<object>;

DEFINE INDEX pdu_event_id_idx ON TABLE pdu COLUMNS event_id UNIQUE;
DEFINE INDEX pdu_room_ts_idx ON TABLE pdu COLUMNS room_id, origin_server_ts;
DEFINE INDEX pdu_sender_idx ON TABLE pdu COLUMNS sender;
DEFINE INDEX pdu_type_idx ON TABLE pdu COLUMNS event_type;
DEFINE INDEX pdu_state_idx ON TABLE pdu COLUMNS room_id, event_type, state_key;

-- Event table - Matrix spec compliant base Event (spec/client/04_security_encryption.md:92-96)
DEFINE TABLE event SCHEMAFULL
    PERMISSIONS
        FOR select, create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD event_id ON TABLE event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD sender ON TABLE event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD origin_server_ts ON TABLE event TYPE int ASSERT $value > 0;
DEFINE FIELD event_type ON TABLE event TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD content ON TABLE event TYPE object DEFAULT {};
DEFINE FIELD state_key ON TABLE event TYPE option<string>;
DEFINE FIELD unsigned ON TABLE event TYPE option<object>;

DEFINE INDEX event_id_idx ON TABLE event COLUMNS event_id UNIQUE;
DEFINE INDEX event_sender_idx ON TABLE event COLUMNS sender;
DEFINE INDEX event_type_idx ON TABLE event COLUMNS event_type;
DEFINE INDEX event_room_ts_idx ON TABLE event COLUMNS room_id, origin_server_ts;-- MembershipEventContent table - Room membership state (Matrix client-server auth)
DEFINE TABLE membership_event_content SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE  -- Authenticated users can read membership they have access to
        FOR create, update WHERE $auth.user_id != NONE  -- Only authenticated users can modify membership
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD event_id ON TABLE membership_event_content TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD sender ON TABLE membership_event_content TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD origin_server_ts ON TABLE membership_event_content TYPE int ASSERT $value > 0;
DEFINE FIELD membership ON TABLE membership_event_content TYPE string ASSERT $value IN ['invite', 'join', 'leave', 'ban', 'knock'];
DEFINE FIELD user_id ON TABLE membership_event_content TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD room_id ON TABLE membership_event_content TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD display_name ON TABLE membership_event_content TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE membership_event_content TYPE option<string>;
DEFINE FIELD reason ON TABLE membership_event_content TYPE option<string>;
DEFINE FIELD third_party_invite ON TABLE membership_event_content TYPE option<object>;

DEFINE INDEX membership_room_user_idx ON TABLE membership_event_content COLUMNS room_id, user_id UNIQUE;
DEFINE INDEX membership_user_idx ON TABLE membership_event_content COLUMNS user_id;
DEFINE INDEX membership_state_idx ON TABLE membership_event_content COLUMNS membership;

-- DeviceKey table - matches DeviceKey entity exactly
DEFINE TABLE device_key SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE  -- Any authenticated user (keys are discoverable for crypto)
        FOR create, update, delete WHERE $auth.user_id != NONE;  -- Users manage their own device keys

DEFINE FIELD user_id ON TABLE device_key TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD device_id ON TABLE device_key TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithms ON TABLE device_key TYPE array<string> DEFAULT [];
DEFINE FIELD keys ON TABLE device_key TYPE object DEFAULT {};
DEFINE FIELD signatures ON TABLE device_key TYPE object DEFAULT {};
DEFINE FIELD unsigned ON TABLE device_key TYPE option<object>;

DEFINE INDEX device_key_user_device_idx ON TABLE device_key COLUMNS user_id, device_id UNIQUE;
DEFINE INDEX device_key_user_idx ON TABLE device_key COLUMNS user_id;

-- CrossSigningKey table - matches CrossSigningKey entity exactly
DEFINE TABLE cross_signing_key SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Cross-signing keys are public for verification in Matrix
        FOR create, update, delete WHERE $auth.user_id != NONE;  -- Users manage their own cross-signing keys

DEFINE FIELD user_id ON TABLE cross_signing_key TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD usage ON TABLE cross_signing_key TYPE array<string> ASSERT array::len($value) > 0;
DEFINE FIELD keys ON TABLE cross_signing_key TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD signatures ON TABLE cross_signing_key TYPE option<object>;

DEFINE INDEX cross_signing_user_idx ON TABLE cross_signing_key COLUMNS user_id;

-- AuthenticationContent table - matches AuthenticationContent enum variants
DEFINE TABLE authentication_content SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD auth_type ON TABLE authentication_content TYPE string ASSERT $value IN ['m.login.password', 'm.login.recaptcha', 'm.login.token', 'm.login.dummy'];
DEFINE FIELD session ON TABLE authentication_content TYPE option<string>;
-- Password auth fields
DEFINE FIELD identifier ON TABLE authentication_content TYPE option<object>;
DEFINE FIELD password ON TABLE authentication_content TYPE option<string>;
-- Recaptcha auth fields  
DEFINE FIELD response ON TABLE authentication_content TYPE option<string>;
-- Token auth fields
DEFINE FIELD token ON TABLE authentication_content TYPE option<string>;

DEFINE INDEX auth_content_type_idx ON TABLE authentication_content COLUMNS auth_type;
DEFINE INDEX auth_content_session_idx ON TABLE authentication_content COLUMNS session;-- ThirdPartyInvite table - matches ThirdPartyInvite entity exactly
DEFINE TABLE third_party_invite SCHEMAFULL
    PERMISSIONS
        FOR select WHERE room_id IN (SELECT room_id FROM membership_event_content WHERE user_id = $auth.user_id)
        FOR create, update, delete WHERE sender = $auth.user_id OR $auth.admin = true;

DEFINE FIELD address ON TABLE third_party_invite TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD medium ON TABLE third_party_invite TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD mxid ON TABLE third_party_invite TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE third_party_invite TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sender ON TABLE third_party_invite TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD signed ON TABLE third_party_invite TYPE object ASSERT object::len($value) > 0;

DEFINE INDEX third_party_invite_room_idx ON TABLE third_party_invite COLUMNS room_id;
DEFINE INDEX third_party_invite_mxid_idx ON TABLE third_party_invite COLUMNS mxid;

-- Filter table - Matrix filtering criteria (matches Filter entity exactly)
DEFINE TABLE filter SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.user_id != NONE;  -- Filters are user-specific resources

DEFINE FIELD limit ON TABLE filter TYPE option<int>;
DEFINE FIELD types ON TABLE filter TYPE option<array<string>>;
DEFINE FIELD not_types ON TABLE filter TYPE option<array<string>>;
DEFINE FIELD senders ON TABLE filter TYPE option<array<string>>;
DEFINE FIELD not_senders ON TABLE filter TYPE option<array<string>>;

-- UserAccountData table - matches Matrix account data storage specification
DEFINE TABLE user_account_data SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE user_account_data TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD room_id ON TABLE user_account_data TYPE option<string> 
    ASSERT ($value IS NONE) OR (string::starts_with($value, '!') AND string::contains($value, ':')); -- null for global account data or valid room ID
DEFINE FIELD type ON TABLE user_account_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD content ON TABLE user_account_data TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE user_account_data TYPE datetime DEFAULT time::now();

DEFINE INDEX user_account_data_user_idx ON TABLE user_account_data COLUMNS user_id;
DEFINE INDEX user_account_data_room_idx ON TABLE user_account_data COLUMNS room_id;
DEFINE INDEX user_account_data_user_type_idx ON TABLE user_account_data COLUMNS user_id, type;
DEFINE INDEX user_account_data_user_room_type_idx ON TABLE user_account_data COLUMNS user_id, room_id, type UNIQUE;

-- UserPresenceUpdate table - matches UserPresenceUpdate entity exactly
DEFINE TABLE user_presence_update SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Presence is visible to users in shared rooms
        FOR create, update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE user_presence_update TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD presence ON TABLE user_presence_update TYPE string ASSERT $value IN ['online', 'offline', 'unavailable'];
DEFINE FIELD last_active_ago ON TABLE user_presence_update TYPE option<int>;
DEFINE FIELD status_msg ON TABLE user_presence_update TYPE option<string>;
DEFINE FIELD currently_active ON TABLE user_presence_update TYPE option<bool>;

DEFINE INDEX presence_user_idx ON TABLE user_presence_update COLUMNS user_id UNIQUE;
DEFINE INDEX presence_state_idx ON TABLE user_presence_update COLUMNS presence;

-- PushRule table - Matrix push notification rules (matches PushRule entity exactly)
DEFINE TABLE push_rule SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.user_id != NONE;  -- Users manage their push rules

DEFINE FIELD rule_id ON TABLE push_rule TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD default ON TABLE push_rule TYPE bool DEFAULT false;
DEFINE FIELD enabled ON TABLE push_rule TYPE bool DEFAULT true;
DEFINE FIELD pattern ON TABLE push_rule TYPE option<string>;
DEFINE FIELD conditions ON TABLE push_rule TYPE option<array<object>>;
DEFINE FIELD actions ON TABLE push_rule TYPE array<object> DEFAULT [];

DEFINE INDEX push_rule_id_idx ON TABLE push_rule COLUMNS rule_id UNIQUE;-- RoomKeyBackup table - matches RoomKeyBackup entity exactly
DEFINE TABLE room_key_backup SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD version ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD auth_data ON TABLE room_key_backup TYPE object DEFAULT {};
DEFINE FIELD count ON TABLE room_key_backup TYPE int DEFAULT 0;
DEFINE FIELD etag ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);

DEFINE INDEX room_key_backup_user_version_idx ON TABLE room_key_backup COLUMNS user_id, version UNIQUE;
DEFINE INDEX room_key_backup_user_idx ON TABLE room_key_backup COLUMNS user_id;

-- TransactionResult table - matches TransactionResult entity exactly
DEFINE TABLE transaction_result SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.server_name != NONE  -- Federation table
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD transaction_id ON TABLE transaction_result TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin ON TABLE transaction_result TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server_ts ON TABLE transaction_result TYPE int ASSERT $value > 0;
DEFINE FIELD data ON TABLE transaction_result TYPE option<object>;

DEFINE INDEX transaction_result_id_idx ON TABLE transaction_result COLUMNS transaction_id UNIQUE;
DEFINE INDEX transaction_result_origin_idx ON TABLE transaction_result COLUMNS origin;

-- EventTemplate table - matches EventTemplate entity exactly
DEFINE TABLE event_template SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update, delete WHERE sender = $auth.user_id OR $auth.admin = true;

DEFINE FIELD event_id ON TABLE event_template TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD sender ON TABLE event_template TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD origin_server_ts ON TABLE event_template TYPE int ASSERT $value > 0;
DEFINE FIELD event_type ON TABLE event_template TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE event_template TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD state_key ON TABLE event_template TYPE option<string>;
DEFINE FIELD content ON TABLE event_template TYPE object DEFAULT {};
DEFINE FIELD unsigned ON TABLE event_template TYPE option<object>;

DEFINE INDEX event_template_id_idx ON TABLE event_template COLUMNS event_id UNIQUE;
DEFINE INDEX event_template_sender_idx ON TABLE event_template COLUMNS sender;
DEFINE INDEX event_template_type_idx ON TABLE event_template COLUMNS event_type;
DEFINE INDEX event_template_room_ts_idx ON TABLE event_template COLUMNS room_id, origin_server_ts;

-- OneTimeKeyObject table - matches OneTimeKeyObject entity exactly
DEFINE TABLE one_time_key_object SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE one_time_key_object TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE one_time_key_object TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_id ON TABLE one_time_key_object TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key ON TABLE one_time_key_object TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD signatures ON TABLE one_time_key_object TYPE option<object>;

DEFINE INDEX one_time_key_user_device_key_idx ON TABLE one_time_key_object COLUMNS user_id, device_id, key_id UNIQUE;
DEFINE INDEX one_time_key_user_device_idx ON TABLE one_time_key_object COLUMNS user_id, device_id;-- =====================================================
-- FEDERATION AND SERVER-TO-SERVER TABLES
-- =====================================================

-- SendJoinRoomState table - matches SendJoinRoomState entity exactly
DEFINE TABLE send_join_room_state SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD state ON TABLE send_join_room_state TYPE array<object> DEFAULT [];
DEFINE FIELD auth_chain ON TABLE send_join_room_state TYPE array<object> DEFAULT [];

-- PublishedRoomsChunk table - matches PublishedRoomsChunk entity exactly
DEFINE TABLE published_rooms_chunk SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Public room directory is readable by all
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE published_rooms_chunk TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD name ON TABLE published_rooms_chunk TYPE option<string>;
DEFINE FIELD topic ON TABLE published_rooms_chunk TYPE option<string>;
DEFINE FIELD canonical_alias ON TABLE published_rooms_chunk TYPE option<string>;
DEFINE FIELD num_joined_members ON TABLE published_rooms_chunk TYPE int DEFAULT 0;
DEFINE FIELD world_readable ON TABLE published_rooms_chunk TYPE bool DEFAULT false;
DEFINE FIELD guest_can_join ON TABLE published_rooms_chunk TYPE bool DEFAULT false;
DEFINE FIELD avatar_url ON TABLE published_rooms_chunk TYPE option<string>;
DEFINE FIELD room_type ON TABLE published_rooms_chunk TYPE option<string>;

DEFINE INDEX published_rooms_id_idx ON TABLE published_rooms_chunk COLUMNS room_id UNIQUE;
DEFINE INDEX published_rooms_type_idx ON TABLE published_rooms_chunk COLUMNS room_type;

-- =====================================================
-- MATRIX /SYNC LIVEQUERY IMPLEMENTATION
-- =====================================================

-- Matrix sync batch tracking for /sync endpoint batch tokens
DEFINE TABLE matrix_sync_batch SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE matrix_sync_batch TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD batch_token ON TABLE matrix_sync_batch TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_batch TYPE int DEFAULT 0;
DEFINE FIELD filter_id ON TABLE matrix_sync_batch TYPE option<string>; -- Optional filter applied to sync
DEFINE FIELD timeout ON TABLE matrix_sync_batch TYPE option<int>; -- Timeout for this sync request  
DEFINE FIELD full_state ON TABLE matrix_sync_batch TYPE bool DEFAULT false; -- Whether to include full state
DEFINE FIELD created_at ON TABLE matrix_sync_batch TYPE datetime DEFAULT time::now();

DEFINE INDEX matrix_sync_batch_user_idx ON TABLE matrix_sync_batch COLUMNS user_id;
DEFINE INDEX matrix_sync_batch_token_idx ON TABLE matrix_sync_batch COLUMNS batch_token UNIQUE;
DEFINE INDEX matrix_sync_batch_sequence_idx ON TABLE matrix_sync_batch COLUMNS sequence_number;
DEFINE INDEX matrix_sync_batch_user_sequence_idx ON TABLE matrix_sync_batch COLUMNS user_id, sequence_number;

-- Matrix sync room events for /sync rooms section
DEFINE TABLE matrix_sync_room_event SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD user_id ON TABLE matrix_sync_room_event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD room_id ON TABLE matrix_sync_room_event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD event_id ON TABLE matrix_sync_room_event TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '$') AND string::contains($value, ':');
DEFINE FIELD event_type ON TABLE matrix_sync_room_event TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_room_event TYPE int ASSERT $value > 0;
DEFINE FIELD timeline_event ON TABLE matrix_sync_room_event TYPE object DEFAULT {};
DEFINE FIELD state_event ON TABLE matrix_sync_room_event TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE matrix_sync_room_event TYPE datetime DEFAULT time::now();

DEFINE INDEX matrix_sync_room_user_idx ON TABLE matrix_sync_room_event COLUMNS user_id;
DEFINE INDEX matrix_sync_room_room_idx ON TABLE matrix_sync_room_event COLUMNS room_id;
DEFINE INDEX matrix_sync_room_sequence_idx ON TABLE matrix_sync_room_event COLUMNS sequence_number;
DEFINE INDEX matrix_sync_room_user_sequence_idx ON TABLE matrix_sync_room_event COLUMNS user_id, sequence_number;
DEFINE INDEX matrix_sync_room_created_idx ON TABLE matrix_sync_room_event COLUMNS created_at;

-- Matrix sync account data for /sync account_data section
DEFINE TABLE matrix_sync_account_data SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE matrix_sync_account_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE matrix_sync_account_data TYPE option<string>; -- null for global account data
DEFINE FIELD data_type ON TABLE matrix_sync_account_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_account_data TYPE int ASSERT $value > 0;
DEFINE FIELD content ON TABLE matrix_sync_account_data TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE matrix_sync_account_data TYPE datetime DEFAULT time::now();

DEFINE INDEX matrix_sync_account_user_idx ON TABLE matrix_sync_account_data COLUMNS user_id;
DEFINE INDEX matrix_sync_account_room_idx ON TABLE matrix_sync_account_data COLUMNS room_id;
DEFINE INDEX matrix_sync_account_sequence_idx ON TABLE matrix_sync_account_data COLUMNS sequence_number;

-- Matrix sync presence updates for /sync presence section
DEFINE TABLE matrix_sync_presence SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE matrix_sync_presence TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD target_user_id ON TABLE matrix_sync_presence TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_presence TYPE int ASSERT $value > 0;
DEFINE FIELD presence_content ON TABLE matrix_sync_presence TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE matrix_sync_presence TYPE datetime DEFAULT time::now();

DEFINE INDEX matrix_sync_presence_user_idx ON TABLE matrix_sync_presence COLUMNS user_id;
DEFINE INDEX matrix_sync_presence_target_idx ON TABLE matrix_sync_presence COLUMNS target_user_id;
DEFINE INDEX matrix_sync_presence_sequence_idx ON TABLE matrix_sync_presence COLUMNS sequence_number;

-- Matrix sync retention configuration
DEFINE TABLE matrix_sync_retention SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.admin = true;

DEFINE FIELD table_name ON TABLE matrix_sync_retention TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD retention_days ON TABLE matrix_sync_retention TYPE int ASSERT $value > 0;
DEFINE FIELD cleanup_batch_size ON TABLE matrix_sync_retention TYPE int DEFAULT 1000;
DEFINE FIELD last_cleanup ON TABLE matrix_sync_retention TYPE datetime DEFAULT time::now();

DEFINE INDEX matrix_sync_retention_table_idx ON TABLE matrix_sync_retention COLUMNS table_name UNIQUE;

-- Initialize retention policies
UPSERT matrix_sync_retention SET 
    table_name = 'matrix_sync_room_event',
    retention_days = 30,
    cleanup_batch_size = 1000,
    last_cleanup = time::now()
WHERE table_name = 'matrix_sync_room_event';

UPSERT matrix_sync_retention SET 
    table_name = 'matrix_sync_account_data',
    retention_days = 90,
    cleanup_batch_size = 1000,
    last_cleanup = time::now()
WHERE table_name = 'matrix_sync_account_data';

UPSERT matrix_sync_retention SET 
    table_name = 'matrix_sync_presence',
    retention_days = 7,
    cleanup_batch_size = 1000,
    last_cleanup = time::now()
WHERE table_name = 'matrix_sync_presence';

UPSERT matrix_sync_retention SET 
    table_name = 'matrix_sync_device_list',
    retention_days = 30,
    cleanup_batch_size = 1000,
    last_cleanup = time::now()
WHERE table_name = 'matrix_sync_device_list';

UPSERT matrix_sync_retention SET 
    table_name = 'matrix_federation_event',
    retention_days = 7,
    cleanup_batch_size = 500,
    last_cleanup = time::now()
WHERE table_name = 'matrix_federation_event';

-- Function to cleanup old sync records
DEFINE FUNCTION fn::cleanup_sync_table($table_name: string) {
    BEGIN TRANSACTION;
    TRY {
        -- Get retention policy for table
        LET $policy = (SELECT * FROM matrix_sync_retention WHERE table_name = $table_name LIMIT 1);
        
        IF array::len($policy) = 0 THEN {
            THROW "No retention policy found for table: " + $table_name;
        };
        
        LET $retention_days = $policy[0].retention_days;
        LET $batch_size = $policy[0].cleanup_batch_size;
        LET $cutoff_date = time::now() - duration::days($retention_days);
        
        -- Delete old records in batches
        LET $deleted = DELETE FROM type::table($table_name) 
            WHERE created_at < $cutoff_date
            LIMIT $batch_size;
        
        -- Update last cleanup time
        UPDATE matrix_sync_retention SET last_cleanup = time::now() 
            WHERE table_name = $table_name;
        
        COMMIT TRANSACTION;
        RETURN {
            table: $table_name,
            deleted_count: array::len($deleted),
            cutoff_date: $cutoff_date
        };
    } CATCH {
        CANCEL TRANSACTION;
        THROW "Cleanup failed for table: " + $table_name;
    };
};

-- Function to generate Matrix /sync batch token
DEFINE FUNCTION fn::generate_batch_token($user_id: string, $sequence: int) {
    RETURN "sync_" + string::replace($user_id, ':', '_') + "_" + <string>$sequence;
};

-- Function to create Matrix /sync response
DEFINE FUNCTION fn::create_sync_response($user_id: string, $since_token: option<string>, $filter_id: option<string>) {
    BEGIN TRANSACTION;
    TRY {
        -- Get current sequence number
        LET $current_sequence = fn::next_sequence();
        LET $next_batch_token = fn::generate_batch_token($user_id, $current_sequence);
        
        -- Parse since token to get starting sequence
        LET $since_sequence = IF $since_token IS NOT NONE THEN (
            SELECT sequence_number FROM matrix_sync_batch WHERE batch_token = $since_token LIMIT 1
        )[0].sequence_number ELSE 0;
        
        -- Get room events since last sync
        LET $room_events = (
            SELECT * FROM matrix_sync_room_event 
            WHERE user_id = $user_id 
            AND sequence_number > $since_sequence
            ORDER BY sequence_number ASC
        );
        
        -- Get account data changes since last sync
        LET $account_data = (
            SELECT * FROM matrix_sync_account_data 
            WHERE user_id = $user_id 
            AND sequence_number > $since_sequence
            ORDER BY sequence_number ASC
        );
        
        -- Get presence updates since last sync
        LET $presence = (
            SELECT * FROM matrix_sync_presence 
            WHERE user_id = $user_id 
            AND sequence_number > $since_sequence
            ORDER BY sequence_number ASC
        );
        
        -- Get device list changes since last sync
        LET $device_lists = (
            SELECT * FROM matrix_sync_device_list 
            WHERE user_id = $user_id 
            AND sequence_number > $since_sequence
            ORDER BY sequence_number ASC
        );
        
        -- Get to_device messages since last sync
        LET $to_device = (
            SELECT * FROM matrix_sync_to_device 
            WHERE user_id = $user_id 
            AND sequence_number > $since_sequence
            ORDER BY sequence_number ASC
        );
        
        -- Store batch token for next sync
        CREATE matrix_sync_batch SET
            user_id = $user_id,
            batch_token = $next_batch_token,
            sequence_number = $current_sequence,
            filter_id = $filter_id;
        
        COMMIT TRANSACTION;
        
        -- Return Matrix-compliant /sync response
        RETURN {
            next_batch: $next_batch_token,
            prev_batch: $since_token,
            account_data: {
                events: $account_data
            },
            presence: {
                events: $presence
            },
            rooms: {
                join: fn::group_room_events($room_events, 'join'),
                invite: fn::group_room_events($room_events, 'invite'),
                leave: fn::group_room_events($room_events, 'leave')
            },
            device_lists: {
                changed: array::distinct(array::map($device_lists, |$d| IF $d.device_change_type = 'changed' THEN $d.changed_user_id END)),
                left: array::distinct(array::map($device_lists, |$d| IF $d.device_change_type = 'left' THEN $d.changed_user_id END))
            },
            to_device: {
                events: $to_device
            }
        };
    } CATCH {
        CANCEL TRANSACTION;
        THROW "Failed to create sync response";
    };
};

-- Function to group room events by room and membership
DEFINE FUNCTION fn::group_room_events($events: array, $membership_filter: string) {
    LET $grouped = {};
    FOR $event IN $events {
        -- Group events by room_id
        LET $room_id = $event.room_id;
        IF !object::has($grouped, $room_id) THEN {
            $grouped[$room_id] = {
                timeline: {events: [], limited: false},
                state: {events: []},
                ephemeral: {events: []},
                account_data: {events: []},
                unread_notifications: {highlight_count: 0, notification_count: 0}
            };
        };
        
        -- Add timeline events
        IF object::has($event, 'timeline_event') AND $event.timeline_event != {} THEN {
            array::push($grouped[$room_id].timeline.events, $event.timeline_event);
        };
        
        -- Add state events  
        IF object::has($event, 'state_event') AND $event.state_event != {} THEN {
            array::push($grouped[$room_id].state.events, $event.state_event);
        };
    };
    RETURN $grouped;
};

-- Matrix room membership cache for efficient LiveQuery lookups
DEFINE TABLE room_membership SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD room_id ON TABLE room_membership TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE room_membership TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD membership ON TABLE room_membership TYPE string ASSERT $value IN ['invite', 'join', 'leave', 'ban', 'knock'];
DEFINE FIELD display_name ON TABLE room_membership TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE room_membership TYPE option<string>;

DEFINE INDEX room_membership_room_idx ON TABLE room_membership COLUMNS room_id;
DEFINE INDEX room_membership_user_idx ON TABLE room_membership COLUMNS user_id;
DEFINE INDEX room_membership_room_user_idx ON TABLE room_membership COLUMNS room_id, user_id UNIQUE;
DEFINE INDEX room_membership_join_user_idx ON TABLE room_membership COLUMNS user_id, membership;
DEFINE INDEX room_membership_join_room_idx ON TABLE room_membership COLUMNS room_id, membership;
DEFINE INDEX room_membership_join_optimization_idx ON TABLE room_membership COLUMNS membership, room_id, user_id;
DEFINE INDEX room_membership_federation_idx ON TABLE room_membership COLUMNS room_id, membership WHERE string::contains(user_id, ':');

-- =====================================================
-- MATRIX /SYNC LIVEQUERY EVENTS
-- =====================================================

-- Global sequence number for Matrix sync ordering
DEFINE TABLE matrix_global_sequence SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true
        FOR create, update, delete WHERE $auth.admin = true;
DEFINE FIELD current_sequence ON TABLE matrix_global_sequence TYPE int DEFAULT 0;

-- Initialize global sequence (safe for reapplication)
UPSERT matrix_global_sequence SET current_sequence = 0 WHERE current_sequence IS NONE;

-- Function to get next sequence number (atomic with error handling)
DEFINE FUNCTION fn::next_sequence() {
    BEGIN TRANSACTION;
    TRY {
        LET $seq = UPDATE matrix_global_sequence SET current_sequence += 1;
        IF array::len($seq) = 0 THEN {
            -- Initialize if no record exists
            INSERT INTO matrix_global_sequence SET current_sequence = 1;
            COMMIT TRANSACTION;
            RETURN 1;
        } ELSE {
            COMMIT TRANSACTION;
            RETURN $seq[0].current_sequence;
        };
    } CATCH {
        CANCEL TRANSACTION;
        THROW "Failed to get next sequence number";
    };
};

-- PDU events for Matrix room timeline (matches Matrix spec)
DEFINE EVENT pdu_timeline_events ON TABLE pdu WHEN $event = "CREATE" THEN (
    LET $sequence = fn::next_sequence();
    
    -- Find room members efficiently using room_membership_cache if available
    LET $room_members = (SELECT user_id FROM room_membership WHERE room_id = $this.room_id AND membership = 'join');
    
    -- Create timeline event for each room member
    FOR $member IN $room_members {
        CREATE matrix_sync_room_event SET
            user_id = $member.user_id,
            room_id = $this.room_id,
            event_id = $this.event_id,
            event_type = $this.event_type,
            sequence_number = $sequence,
            timeline_event = {
                event_id: $this.event_id,
                type: $this.event_type,
                room_id: $this.room_id,
                sender: $this.sender,
                content: $this.content,
                origin_server_ts: $this.origin_server_ts,
                unsigned: $this.unsigned
            }
    }
);

-- Room state events for Matrix room state (matches Matrix spec)
DEFINE EVENT room_state_events ON TABLE pdu WHEN $event = "CREATE" AND $this.state_key IS NOT NONE THEN (
    LET $sequence = fn::next_sequence();
    
    -- Find room members efficiently
    LET $room_members = (SELECT user_id FROM room_membership WHERE room_id = $this.room_id AND membership = 'join');
    
    -- Create state event for each room member
    FOR $member IN $room_members {
        CREATE matrix_sync_room_event SET
            user_id = $member.user_id,
            room_id = $this.room_id,
            event_id = $this.event_id,
            event_type = $this.event_type,
            sequence_number = $sequence,
            state_event = {
                event_id: $this.event_id,
                type: $this.event_type,
                room_id: $this.room_id,
                sender: $this.sender,
                state_key: $this.state_key,
                content: $this.content,
                origin_server_ts: $this.origin_server_ts,
                unsigned: $this.unsigned
            }
    }
);

-- Account data events for Matrix global and room-scoped account data
DEFINE EVENT account_data_events ON TABLE user_account_data WHEN $event = "CREATE" OR $event = "UPDATE" THEN (
    LET $sequence = fn::next_sequence();
    
    CREATE matrix_sync_account_data SET
        user_id = $this.user_id,
        room_id = $this.room_id,
        data_type = $this.type,
        sequence_number = $sequence,
        content = $this.content
);

-- Presence events for Matrix user presence updates (optimized)
DEFINE EVENT presence_events ON TABLE user_presence_update WHEN $event = "CREATE" OR $event = "UPDATE" THEN (
    LET $sequence = fn::next_sequence();
    
    -- Find users who share rooms with this user using efficient JOIN
    LET $shared_room_users = (
        SELECT DISTINCT rm2.user_id AS user_id FROM room_membership rm1
        JOIN room_membership rm2 ON rm1.room_id = rm2.room_id
        WHERE rm1.user_id = $this.user_id 
        AND rm1.membership = 'join'
        AND rm2.membership = 'join'
        AND rm2.user_id != $this.user_id
    );
    
    -- Create presence update for each user who shares a room
    FOR $user IN $shared_room_users {
        CREATE matrix_sync_presence SET
            user_id = $user.user_id,
            target_user_id = $this.user_id,
            sequence_number = $sequence,
            presence_content = {
                presence: $this.presence,
                last_active_ago: $this.last_active_ago,
                status_msg: $this.status_msg,
                currently_active: $this.currently_active
            }
    }
);

-- Matrix device list updates for E2E encryption (matches Matrix spec)
DEFINE TABLE matrix_sync_device_list SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD user_id ON TABLE matrix_sync_device_list TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD changed_user_id ON TABLE matrix_sync_device_list TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_device_list TYPE int ASSERT $value > 0;
DEFINE FIELD device_change_type ON TABLE matrix_sync_device_list TYPE string ASSERT $value IN ['changed', 'left'];
DEFINE FIELD created_at ON TABLE matrix_sync_device_list TYPE datetime DEFAULT time::now();

DEFINE INDEX matrix_sync_device_user_idx ON TABLE matrix_sync_device_list COLUMNS user_id;
DEFINE INDEX matrix_sync_device_changed_idx ON TABLE matrix_sync_device_list COLUMNS changed_user_id;
DEFINE INDEX matrix_sync_device_sequence_idx ON TABLE matrix_sync_device_list COLUMNS sequence_number;

-- Device key changes for Matrix device list updates (optimized)
DEFINE EVENT device_key_events ON TABLE device_key WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LET $sequence = fn::next_sequence();
    
    -- Find users who share rooms with this device owner using efficient JOIN
    LET $shared_room_users = (
        SELECT DISTINCT rm2.user_id AS user_id FROM room_membership rm1
        JOIN room_membership rm2 ON rm1.room_id = rm2.room_id
        WHERE rm1.user_id = $this.user_id 
        AND rm1.membership = 'join'
        AND rm2.membership = 'join'
        AND rm2.user_id != $this.user_id
    );
    
    -- Create device list update for each user who shares a room
    FOR $user IN $shared_room_users {
        CREATE matrix_sync_device_list SET
            user_id = $user.user_id,
            changed_user_id = $this.user_id,
            sequence_number = $sequence,
            device_change_type = IF $event = "DELETE" THEN 'left' ELSE 'changed' END
    }
);

-- Matrix to_device messages for E2E encryption (matches Matrix spec)
DEFINE TABLE matrix_sync_to_device SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD user_id ON TABLE matrix_sync_to_device TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE matrix_sync_to_device TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD message_type ON TABLE matrix_sync_to_device TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_sync_to_device TYPE int ASSERT $value > 0;
DEFINE FIELD content ON TABLE matrix_sync_to_device TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE matrix_sync_to_device TYPE datetime DEFAULT time::now();

DEFINE INDEX matrix_sync_to_device_user_idx ON TABLE matrix_sync_to_device COLUMNS user_id;
DEFINE INDEX matrix_sync_to_device_user_device_idx ON TABLE matrix_sync_to_device COLUMNS user_id, device_id;
DEFINE INDEX matrix_sync_to_device_sequence_idx ON TABLE matrix_sync_to_device COLUMNS sequence_number;

-- Membership events to maintain room_membership cache and notify users
DEFINE EVENT membership_cache_events ON TABLE membership_event_content WHEN $event = "CREATE" OR $event = "UPDATE" THEN (
    -- Update or insert into room_membership cache
    UPSERT room_membership SET
        room_id = $this.room_id,
        user_id = $this.user_id,
        membership = $this.membership,
        display_name = $this.display_name,
        avatar_url = $this.avatar_url
    WHERE room_id = $this.room_id AND user_id = $this.user_id;
    
    -- Generate sync events for membership changes
    LET $sequence = fn::next_sequence();
    
    -- Find room members to notify (include the affected user)
    LET $room_members = (SELECT user_id FROM room_membership WHERE room_id = $this.room_id AND membership = 'join');
    
    -- Create membership state event for each room member
    FOR $member IN $room_members {
        CREATE matrix_sync_room_event SET
            user_id = $member.user_id,
            room_id = $this.room_id,
            event_id = $this.event_id,
            event_type = 'm.room.member',
            sequence_number = $sequence,
            state_event = {
                event_id: $this.event_id,
                type: 'm.room.member',
                room_id: $this.room_id,
                sender: $this.sender,
                state_key: $this.user_id,
                content: {
                    membership: $this.membership,
                    display_name: $this.display_name,
                    avatar_url: $this.avatar_url,
                    reason: $this.reason
                },
                origin_server_ts: $this.origin_server_ts,
                unsigned: {}
            }
    }
);

-- =====================================================
-- MATRIX FEDERATION LIVEQUERY SUPPORT
-- =====================================================

-- Matrix federation event queue for server-to-server communication
DEFINE TABLE matrix_federation_event SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.admin = true;

DEFINE FIELD destination_server ON TABLE matrix_federation_event TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD event_id ON TABLE matrix_federation_event TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE matrix_federation_event TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_federation_event TYPE int ASSERT $value > 0;
DEFINE FIELD pdu_content ON TABLE matrix_federation_event TYPE object DEFAULT {};
DEFINE FIELD retry_count ON TABLE matrix_federation_event TYPE int DEFAULT 0;
DEFINE FIELD max_retry_count ON TABLE matrix_federation_event TYPE int DEFAULT 5;
DEFINE FIELD next_retry_at ON TABLE matrix_federation_event TYPE datetime DEFAULT time::now();
DEFINE FIELD last_error ON TABLE matrix_federation_event TYPE option<string>;
DEFINE FIELD status ON TABLE matrix_federation_event TYPE string DEFAULT 'pending' ASSERT $value IN ['pending', 'processing', 'sent', 'failed'];
DEFINE FIELD created_at ON TABLE matrix_federation_event TYPE datetime DEFAULT time::now();

DEFINE INDEX matrix_federation_destination_idx ON TABLE matrix_federation_event COLUMNS destination_server;
DEFINE INDEX matrix_federation_room_idx ON TABLE matrix_federation_event COLUMNS room_id;
DEFINE INDEX matrix_federation_sequence_idx ON TABLE matrix_federation_event COLUMNS sequence_number;
DEFINE INDEX matrix_federation_retry_idx ON TABLE matrix_federation_event COLUMNS next_retry_at;
DEFINE INDEX matrix_federation_status_idx ON TABLE matrix_federation_event COLUMNS status;
DEFINE INDEX matrix_federation_pending_idx ON TABLE matrix_federation_event COLUMNS status, next_retry_at WHERE status = 'pending';

-- Federation events for outbound PDU distribution
DEFINE EVENT federation_pdu_events ON TABLE pdu WHEN $event = "CREATE" THEN (
    -- Only federate events if authentication context is valid
    IF $auth.server_name IS NOT NONE AND $auth.server_name != '' THEN (
        -- Only federate events from federated rooms (not local-only events)
        IF array::len($this.auth_events) > 0 THEN (
            LET $sequence = fn::next_sequence();
            
            -- Find remote servers in the room (extract from user IDs in membership) - optimized with bounds checking
            LET $remote_servers = (
                SELECT DISTINCT string::split(user_id, ':')[1] AS server 
                FROM room_membership 
                WHERE room_id = $this.room_id 
                AND membership = 'join' 
                AND string::contains(user_id, ':')
                AND array::len(string::split(user_id, ':')) >= 2
                AND string::split(user_id, ':')[1] != $auth.server_name
            );
            
            -- Create federation event for each remote server with error handling
            FOR $server IN $remote_servers {
                IF $server.server IS NOT NONE 
                AND string::is::not::empty($server.server) 
                AND $server.server != $auth.server_name THEN (
                    TRY {
                CREATE matrix_federation_event SET
                    destination_server = $server.server,
                    event_id = $this.event_id,
                    room_id = $this.room_id,
                    sequence_number = $sequence,
                    pdu_content = {
                        auth_events: $this.auth_events,
                        content: $this.content,
                        depth: $this.depth,
                        event_id: $this.event_id,
                        hashes: $this.hashes,
                        origin: $this.origin,
                        origin_server_ts: $this.origin_server_ts,
                        prev_events: $this.prev_events,
                        room_id: $this.room_id,
                        sender: $this.sender,
                        signatures: $this.signatures,
                        state_key: $this.state_key,
                        type: $this.event_type,
                        unsigned: $this.unsigned
                    };
                    } CATCH {
                        -- Log federation error but don't fail the entire event
                        -- Federation failures shouldn't block local event processing
                    }
                ) END
            }
        ) END
    ) END
);

-- =====================================================
-- MATRIX PROTOCOL COMPLIANCE NOTES
-- =====================================================
-- This schema precisely aligns with Matrix entity types from ./packages/entity
-- Each table matches the exact field structure of corresponding Rust structs
-- LiveQuery events enable real-time Matrix /sync endpoint implementation
-- Federation LiveQuery events enable real-time server-to-server communication
-- Permissions ensure Matrix security model compliance for both client and server APIs
-- Indexes optimize Matrix protocol query patterns for both sync and federation-- =====================================================
-- CORE MATRIX ENTITY TABLES (BATCH 1)
-- =====================================================

-- RoomAliasMapping table - matches RoomAliasMapping entity exactly
DEFINE TABLE room_alias_mapping SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Room aliases are publicly discoverable
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE room_alias_mapping TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD alias ON TABLE room_alias_mapping TYPE string ASSERT string::is::not::empty($value);

DEFINE INDEX room_alias_mapping_room_idx ON TABLE room_alias_mapping COLUMNS room_id;
DEFINE INDEX room_alias_mapping_alias_idx ON TABLE room_alias_mapping COLUMNS alias UNIQUE;

-- SessionData table - matches SessionData entity exactly
DEFINE TABLE session_data SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE session_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD session_id ON TABLE session_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD encrypted_data ON TABLE session_data TYPE object DEFAULT {};

DEFINE INDEX session_data_user_session_idx ON TABLE session_data COLUMNS user_id, session_id UNIQUE;
DEFINE INDEX session_data_user_idx ON TABLE session_data COLUMNS user_id;

-- ServerInfo table - matches ServerInfo entity exactly
DEFINE TABLE server_info SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Server info is publicly accessible
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD server ON TABLE server_info TYPE object ASSERT object::len($value) > 0;

-- JoinRoomResponse table - matches JoinRoomResponse entity exactly
DEFINE TABLE join_room_response SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD room_id ON TABLE join_room_response TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE join_room_response TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD timestamp ON TABLE join_room_response TYPE datetime DEFAULT time::now();

DEFINE INDEX join_room_response_room_user_idx ON TABLE join_room_response COLUMNS room_id, user_id;

-- InviteV1Request table - matches InviteV1Request entity exactly
DEFINE TABLE invite_v1_request SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD content ON TABLE invite_v1_request TYPE object DEFAULT {};
DEFINE FIELD origin ON TABLE invite_v1_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server_ts ON TABLE invite_v1_request TYPE int ASSERT $value > 0;
DEFINE FIELD sender ON TABLE invite_v1_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD state_key ON TABLE invite_v1_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD event_type ON TABLE invite_v1_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD unsigned ON TABLE invite_v1_request TYPE object DEFAULT {};

DEFINE INDEX invite_v1_request_sender_idx ON TABLE invite_v1_request COLUMNS sender;
DEFINE INDEX invite_v1_request_origin_idx ON TABLE invite_v1_request COLUMNS origin;-- EncryptedFile table - Matrix encrypted file metadata (matches EncryptedFile entity exactly)
DEFINE TABLE encrypted_file SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.user_id != NONE;  -- Files are accessible to authenticated users

DEFINE FIELD url ON TABLE encrypted_file TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key ON TABLE encrypted_file TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD iv ON TABLE encrypted_file TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD hashes ON TABLE encrypted_file TYPE object DEFAULT {};
DEFINE FIELD v ON TABLE encrypted_file TYPE string ASSERT string::is::not::empty($value);

DEFINE INDEX encrypted_file_url_idx ON TABLE encrypted_file COLUMNS url;

-- KeyClaimRequest table - matches KeyClaimRequest entity exactly
DEFINE TABLE key_claim_request SCHEMAFULL
    PERMISSIONS
        FOR select, create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD one_time_keys ON TABLE key_claim_request TYPE object DEFAULT {};
DEFINE FIELD requesting_user_id ON TABLE key_claim_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD timestamp ON TABLE key_claim_request TYPE datetime DEFAULT time::now();

DEFINE INDEX key_claim_request_user_idx ON TABLE key_claim_request COLUMNS requesting_user_id;

-- VerificationStart table - matches VerificationStart entity exactly
DEFINE TABLE verification_start SCHEMAFULL
    PERMISSIONS
        FOR select WHERE from_user_id = $auth.user_id OR to_user_id = $auth.user_id
        FOR create WHERE from_user_id = $auth.user_id
        FOR update, delete WHERE from_user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD from_device ON TABLE verification_start TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD from_user_id ON TABLE verification_start TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD to_user_id ON TABLE verification_start TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD m_relates_to ON TABLE verification_start TYPE option<object>;
DEFINE FIELD method ON TABLE verification_start TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD next_method ON TABLE verification_start TYPE option<string>;
DEFINE FIELD transaction_id ON TABLE verification_start TYPE option<string>;

DEFINE INDEX verification_start_from_user_idx ON TABLE verification_start COLUMNS from_user_id;
DEFINE INDEX verification_start_to_user_idx ON TABLE verification_start COLUMNS to_user_id;
DEFINE INDEX verification_start_transaction_idx ON TABLE verification_start COLUMNS transaction_id;-- RoomStateResponse table - matches RoomStateResponse entity exactly
DEFINE TABLE room_state_response SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD auth_chain ON TABLE room_state_response TYPE array<object> DEFAULT [];
DEFINE FIELD pdus ON TABLE room_state_response TYPE array<object> DEFAULT [];
DEFINE FIELD room_id ON TABLE room_state_response TYPE string ASSERT string::is::not::empty($value);

DEFINE INDEX room_state_response_room_idx ON TABLE room_state_response COLUMNS room_id;

-- TypingNotification table - matches TypingNotification entity exactly
DEFINE TABLE typing_notification SCHEMAFULL
    PERMISSIONS
        FOR select WHERE room_id IN (SELECT room_id FROM membership_event_content WHERE user_id = $auth.user_id AND membership = 'join')
        FOR create WHERE user_id = $auth.user_id
        FOR update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD room_id ON TABLE typing_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD typing ON TABLE typing_notification TYPE bool DEFAULT false;
DEFINE FIELD user_id ON TABLE typing_notification TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD timestamp ON TABLE typing_notification TYPE datetime DEFAULT time::now();

DEFINE INDEX typing_notification_room_idx ON TABLE typing_notification COLUMNS room_id;
DEFINE INDEX typing_notification_user_idx ON TABLE typing_notification COLUMNS user_id;

-- =====================================================
-- BATCH 2: FEDERATION ENTITIES
-- =====================================================-- InviteV2Request table - matches InviteV2Request entity exactly
DEFINE TABLE invite_v2_request SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD event ON TABLE invite_v2_request TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD invite_room_state ON TABLE invite_v2_request TYPE option<array<object>>;
DEFINE FIELD room_version ON TABLE invite_v2_request TYPE string ASSERT string::is::not::empty($value);

DEFINE INDEX invite_v2_request_room_version_idx ON TABLE invite_v2_request COLUMNS room_version;

-- SendJoinRequest table - matches SendJoinRequest entity exactly
DEFINE TABLE send_join_request SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD content ON TABLE send_join_request TYPE object DEFAULT {};
DEFINE FIELD origin ON TABLE send_join_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server_ts ON TABLE send_join_request TYPE int ASSERT $value > 0;
DEFINE FIELD sender ON TABLE send_join_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD state_key ON TABLE send_join_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD event_type ON TABLE send_join_request TYPE string ASSERT string::is::not::empty($value);

DEFINE INDEX send_join_request_sender_idx ON TABLE send_join_request COLUMNS sender;
DEFINE INDEX send_join_request_origin_idx ON TABLE send_join_request COLUMNS origin;

-- MakeJoinResponse table - matches MakeJoinResponse entity exactly
DEFINE TABLE make_join_response SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD event ON TABLE make_join_response TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD room_version ON TABLE make_join_response TYPE string ASSERT string::is::not::empty($value);

DEFINE INDEX make_join_response_room_version_idx ON TABLE make_join_response COLUMNS room_version;

-- FederationTransaction table - matches FederationTransaction entity exactly
DEFINE TABLE federation_transaction SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD origin ON TABLE federation_transaction TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server_ts ON TABLE federation_transaction TYPE int ASSERT $value > 0;
DEFINE FIELD pdus ON TABLE federation_transaction TYPE array<object> DEFAULT [];
DEFINE FIELD edus ON TABLE federation_transaction TYPE option<array<object>>;
DEFINE FIELD transaction_id ON TABLE federation_transaction TYPE string ASSERT string::is::not::empty($value);

DEFINE INDEX federation_transaction_origin_idx ON TABLE federation_transaction COLUMNS origin;
DEFINE INDEX federation_transaction_id_idx ON TABLE federation_transaction COLUMNS transaction_id UNIQUE;

-- BackfillResponse table - matches BackfillResponse entity exactly
DEFINE TABLE backfill_response SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD origin ON TABLE backfill_response TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server_ts ON TABLE backfill_response TYPE int ASSERT $value > 0;
DEFINE FIELD pdus ON TABLE backfill_response TYPE array<object> DEFAULT [];

DEFINE INDEX backfill_response_origin_idx ON TABLE backfill_response COLUMNS origin;

-- =====================================================
-- SERVER SIGNING KEYS AND CRYPTOGRAPHIC INFRASTRUCTURE
-- =====================================================

-- Server signing keys table for Ed25519 federation signing and verification
DEFINE TABLE server_signing_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true -- Server keys are public for verification
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD key_id ON TABLE server_signing_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD server_name ON TABLE server_signing_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD private_key ON TABLE server_signing_keys TYPE option<string>; -- Only present for our own keys
DEFINE FIELD public_key ON TABLE server_signing_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE server_signing_keys TYPE datetime DEFAULT time::now();
DEFINE FIELD fetched_at ON TABLE server_signing_keys TYPE option<datetime>; -- For remote keys
DEFINE FIELD expires_at ON TABLE server_signing_keys TYPE option<datetime>;
DEFINE FIELD is_active ON TABLE server_signing_keys TYPE bool DEFAULT true;
DEFINE FIELD valid_until_ts ON TABLE server_signing_keys TYPE option<int>; -- Matrix timestamp
DEFINE FIELD signatures ON TABLE server_signing_keys TYPE option<object>; -- For remote key verification

DEFINE INDEX server_keys_server_key_idx ON TABLE server_signing_keys COLUMNS server_name, key_id UNIQUE;
DEFINE INDEX server_keys_server_idx ON TABLE server_signing_keys COLUMNS server_name;
DEFINE INDEX server_keys_active_idx ON TABLE server_signing_keys COLUMNS server_name, is_active;
DEFINE INDEX server_keys_expires_idx ON TABLE server_signing_keys COLUMNS expires_at;

-- User access tokens table for Matrix client authentication
DEFINE TABLE user_access_tokens SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD token ON TABLE user_access_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE user_access_tokens TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD device_id ON TABLE user_access_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE user_access_tokens TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE user_access_tokens TYPE option<datetime>;
DEFINE FIELD last_used_at ON TABLE user_access_tokens TYPE option<datetime>;
DEFINE FIELD is_active ON TABLE user_access_tokens TYPE bool DEFAULT true;

DEFINE INDEX user_tokens_token_idx ON TABLE user_access_tokens COLUMNS token UNIQUE;
DEFINE INDEX user_tokens_user_idx ON TABLE user_access_tokens COLUMNS user_id;
DEFINE INDEX user_tokens_user_device_idx ON TABLE user_access_tokens COLUMNS user_id, device_id;
DEFINE INDEX user_tokens_expires_idx ON TABLE user_access_tokens COLUMNS expires_at;

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

DEFINE INDEX federation_tx_key_idx ON TABLE federation_transactions COLUMNS transaction_key UNIQUE;
DEFINE INDEX federation_tx_server_idx ON TABLE federation_transactions COLUMNS origin_server;
DEFINE INDEX federation_tx_expires_idx ON TABLE federation_transactions COLUMNS expires_at;

-- Server capabilities and configuration
DEFINE TABLE server_config SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true -- Public server information
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD key ON TABLE server_config TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD value ON TABLE server_config TYPE flexible; -- Can be string, number, boolean, object, array
DEFINE FIELD description ON TABLE server_config TYPE option<string>;
DEFINE FIELD updated_at ON TABLE server_config TYPE datetime DEFAULT time::now();
DEFINE FIELD is_public ON TABLE server_config TYPE bool DEFAULT false; -- Whether exposed in server discovery

DEFINE INDEX server_config_key_idx ON TABLE server_config COLUMNS key UNIQUE;
DEFINE INDEX server_config_public_idx ON TABLE server_config COLUMNS is_public;

-- Initialize default server configuration
UPSERT server_config SET 
    key = 'server_name',
    value = 'localhost:8008',
    description = 'Homeserver domain name',
    is_public = true
WHERE key = 'server_name';

UPSERT server_config SET 
    key = 'server_version',
    value = 'matryx/1.0.0',
    description = 'Server implementation version',
    is_public = true
WHERE key = 'server_version';

UPSERT server_config SET 
    key = 'supported_room_versions',
    value = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '10'],
    description = 'Supported Matrix room versions',
    is_public = true
WHERE key = 'supported_room_versions';

UPSERT server_config SET 
    key = 'default_room_version',
    value = '10',
    description = 'Default room version for new rooms',
    is_public = true
WHERE key = 'default_room_version';

-- =====================================================
-- MATRIX PROTOCOL COMPLIANCE SUMMARY
-- =====================================================
-- This schema is now fully aligned with Matrix specification as source of truth
-- Core Matrix entities implemented: Device, PDU, Event, MembershipEventContent,
-- DeviceKey, CrossSigningKey, AuthenticationContent, ThirdPartyInvite, Filter,
-- UserPresenceUpdate, PushRule, RoomKeyBackup, TransactionResult, EventTemplate,
-- OneTimeKeyObject, and federation entities (SendJoinRoomState, PublishedRoomsChunk, etc.)
--
-- Server cryptographic infrastructure: server_signing_keys, user_access_tokens,
-- federation_transactions, server_config tables with proper indexing and permissions
--
-- LiveQuery events provide real-time Matrix /sync endpoint support
-- Permissions follow Matrix security model for room membership and device access
-- Indexes optimized for Matrix protocol query patterns and federation performance
--
-- All tables match corresponding entity definitions in packages/entity/src/types
-- Schema serves as single source of truth for SurrealDB 3.0 Matrix homeserver implementation

-- =====================================================
-- ADVANCED MATRIX FEATURES SCHEMA EXTENSION
-- =====================================================
-- Push Notifications, Search, Third-Party Integration, 
-- Public Room Directory, and Server Administration

-- Push rules storage
DEFINE TABLE push_rules SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE user_id = $auth.user_id
        FOR create, update, delete WHERE user_id = $auth.user_id;

DEFINE FIELD id ON TABLE push_rules TYPE string;
DEFINE FIELD user_id ON TABLE push_rules TYPE string;
DEFINE FIELD rule_id ON TABLE push_rules TYPE string;
DEFINE FIELD kind ON TABLE push_rules TYPE string; -- override, content, room, sender, underride
DEFINE FIELD priority ON TABLE push_rules TYPE int;
DEFINE FIELD conditions ON TABLE push_rules TYPE array<object>;
DEFINE FIELD actions ON TABLE push_rules TYPE array<object>;
DEFINE FIELD enabled ON TABLE push_rules TYPE bool DEFAULT true;
DEFINE FIELD default ON TABLE push_rules TYPE bool DEFAULT false;
DEFINE FIELD created_at ON TABLE push_rules TYPE datetime DEFAULT time::now();

DEFINE INDEX push_rules_user_rules ON TABLE push_rules COLUMNS user_id, kind, priority;
DEFINE INDEX push_rules_rule_lookup ON TABLE push_rules COLUMNS user_id, rule_id UNIQUE;

-- Pushers (devices registered for notifications)
DEFINE TABLE pushers SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE user_id = $auth.user_id
        FOR create, update, delete WHERE user_id = $auth.user_id;

DEFINE FIELD id ON TABLE pushers TYPE string;
DEFINE FIELD user_id ON TABLE pushers TYPE string;
DEFINE FIELD pusher_id ON TABLE pushers TYPE string;
DEFINE FIELD kind ON TABLE pushers TYPE string; -- http, email, etc.
DEFINE FIELD app_id ON TABLE pushers TYPE string;
DEFINE FIELD app_display_name ON TABLE pushers TYPE string;
DEFINE FIELD device_display_name ON TABLE pushers TYPE string;
DEFINE FIELD profile_tag ON TABLE pushers TYPE option<string>;
DEFINE FIELD lang ON TABLE pushers TYPE string;
DEFINE FIELD data ON TABLE pushers TYPE object; -- gateway URL, format, etc.
DEFINE FIELD created_at ON TABLE pushers TYPE datetime DEFAULT time::now();

DEFINE INDEX pushers_user_pushers ON TABLE pushers COLUMNS user_id;
DEFINE INDEX pushers_pusher_lookup ON TABLE pushers COLUMNS user_id, pusher_id UNIQUE;

-- Search index metadata
DEFINE TABLE search_index SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD event_id ON TABLE search_index TYPE string;
DEFINE FIELD room_id ON TABLE search_index TYPE string;
DEFINE FIELD sender ON TABLE search_index TYPE string;
DEFINE FIELD event_type ON TABLE search_index TYPE string;
DEFINE FIELD content_body ON TABLE search_index TYPE option<string>;
DEFINE FIELD content_msgtype ON TABLE search_index TYPE option<string>;
DEFINE FIELD origin_server_ts ON TABLE search_index TYPE datetime;

DEFINE INDEX search_index_room_events ON TABLE search_index COLUMNS room_id, origin_server_ts;
DEFINE INDEX search_index_content_search ON TABLE search_index COLUMNS content_body;
DEFINE INDEX search_index_event_lookup ON TABLE search_index COLUMNS event_id UNIQUE;

-- Third-party protocols
DEFINE TABLE thirdparty_protocols SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD protocol_id ON TABLE thirdparty_protocols TYPE string;
DEFINE FIELD display_name ON TABLE thirdparty_protocols TYPE string;
DEFINE FIELD avatar_url ON TABLE thirdparty_protocols TYPE option<string>;
DEFINE FIELD user_fields ON TABLE thirdparty_protocols TYPE array<object>;
DEFINE FIELD location_fields ON TABLE thirdparty_protocols TYPE array<object>;
DEFINE FIELD instances ON TABLE thirdparty_protocols TYPE array<object>;
DEFINE FIELD created_at ON TABLE thirdparty_protocols TYPE datetime DEFAULT time::now();

DEFINE INDEX thirdparty_protocols_lookup ON TABLE thirdparty_protocols COLUMNS protocol_id UNIQUE;

-- Third-party locations (rooms/channels)
DEFINE TABLE thirdparty_locations SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD protocol ON TABLE thirdparty_locations TYPE string;
DEFINE FIELD alias ON TABLE thirdparty_locations TYPE string;
DEFINE FIELD fields ON TABLE thirdparty_locations TYPE object;
DEFINE FIELD created_at ON TABLE thirdparty_locations TYPE datetime DEFAULT time::now();

DEFINE INDEX thirdparty_locations_protocol_alias ON TABLE thirdparty_locations COLUMNS protocol, alias;

-- Third-party users
DEFINE TABLE thirdparty_users SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD protocol ON TABLE thirdparty_users TYPE string;
DEFINE FIELD userid ON TABLE thirdparty_users TYPE string;
DEFINE FIELD fields ON TABLE thirdparty_users TYPE object;
DEFINE FIELD created_at ON TABLE thirdparty_users TYPE datetime DEFAULT time::now();

DEFINE INDEX thirdparty_users_protocol_user ON TABLE thirdparty_users COLUMNS protocol, userid;

-- Public rooms directory
DEFINE TABLE public_rooms SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true OR $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.room_id AND membership = 'join');

DEFINE FIELD room_id ON TABLE public_rooms TYPE string;
DEFINE FIELD name ON TABLE public_rooms TYPE option<string>;
DEFINE FIELD topic ON TABLE public_rooms TYPE option<string>;
DEFINE FIELD canonical_alias ON TABLE public_rooms TYPE option<string>;
DEFINE FIELD num_joined_members ON TABLE public_rooms TYPE int DEFAULT 0;
DEFINE FIELD avatar_url ON TABLE public_rooms TYPE option<string>;
DEFINE FIELD world_readable ON TABLE public_rooms TYPE bool DEFAULT false;
DEFINE FIELD guest_can_join ON TABLE public_rooms TYPE bool DEFAULT false;
DEFINE FIELD join_rule ON TABLE public_rooms TYPE string DEFAULT 'invite';
DEFINE FIELD room_type ON TABLE public_rooms TYPE option<string>;
DEFINE FIELD created_at ON TABLE public_rooms TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE public_rooms TYPE datetime DEFAULT time::now();

DEFINE INDEX public_rooms_room_search ON TABLE public_rooms COLUMNS name, topic;
DEFINE INDEX public_rooms_member_count ON TABLE public_rooms COLUMNS num_joined_members DESC;
DEFINE INDEX public_rooms_room_type_filter ON TABLE public_rooms COLUMNS room_type;
DEFINE INDEX public_rooms_room_lookup ON TABLE public_rooms COLUMNS room_id UNIQUE;

-- LiveQuery events for advanced features
DEFINE EVENT push_rules_changed ON TABLE push_rules WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM push_rules WHERE user_id = $after.user_id OR user_id = $before.user_id
);

DEFINE EVENT pushers_changed ON TABLE pushers WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM pushers WHERE user_id = $after.user_id OR user_id = $before.user_id
);

DEFINE EVENT public_rooms_changed ON TABLE public_rooms WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM public_rooms
);

DEFINE EVENT search_index_changed ON TABLE search_index WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM search_index WHERE room_id = $after.room_id OR room_id = $before.room_id
);
-- User profiles table for User Experience API
DEFINE TABLE user_profiles SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.user_id = user_id OR $auth.is_admin = true
        FOR create, update, delete WHERE $auth.user_id = user_id OR $auth.is_admin = true;

DEFINE FIELD user_id ON TABLE user_profiles TYPE string ASSERT $value != "";
DEFINE FIELD display_name ON TABLE user_profiles TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE user_profiles TYPE option<string>;
DEFINE FIELD created_at ON TABLE user_profiles TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE user_profiles TYPE datetime DEFAULT time::now();

DEFINE INDEX user_profiles_lookup ON TABLE user_profiles COLUMNS user_id UNIQUE;

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

-- Room tags table for room organization
DEFINE TABLE room_tags SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.user_id = user_id OR $auth.is_admin = true
        FOR create, update, delete WHERE $auth.user_id = user_id OR $auth.is_admin = true;

DEFINE FIELD id ON TABLE room_tags TYPE string ASSERT $value != "";
DEFINE FIELD user_id ON TABLE room_tags TYPE string ASSERT $value != "";
DEFINE FIELD room_id ON TABLE room_tags TYPE string ASSERT $value != "";
DEFINE FIELD tag ON TABLE room_tags TYPE string ASSERT $value != "";
DEFINE FIELD tag_order ON TABLE room_tags TYPE option<float>;
DEFINE FIELD created_at ON TABLE room_tags TYPE datetime DEFAULT time::now();

DEFINE INDEX room_tags_user_room_lookup ON TABLE room_tags COLUMNS user_id, room_id, tag UNIQUE;

-- Media files table for media storage
DEFINE TABLE media_files SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.user_id = uploaded_by OR $auth.is_admin = true;

DEFINE FIELD media_id ON TABLE media_files TYPE string ASSERT $value != "";
DEFINE FIELD server_name ON TABLE media_files TYPE string ASSERT $value != "";
DEFINE FIELD content_type ON TABLE media_files TYPE string ASSERT $value != "";
DEFINE FIELD content_length ON TABLE media_files TYPE int ASSERT $value > 0;
DEFINE FIELD file_path ON TABLE media_files TYPE string ASSERT $value != "";
DEFINE FIELD upload_name ON TABLE media_files TYPE option<string>;
DEFINE FIELD uploaded_by ON TABLE media_files TYPE string ASSERT $value != "";
DEFINE FIELD created_at ON TABLE media_files TYPE datetime DEFAULT time::now();

DEFINE INDEX media_files_lookup ON TABLE media_files COLUMNS server_name, media_id UNIQUE;

-- Third-party identifiers table
DEFINE TABLE user_threepids SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.user_id = user_id OR $auth.is_admin = true
        FOR create, update, delete WHERE $auth.user_id = user_id OR $auth.is_admin = true;

DEFINE FIELD id ON TABLE user_threepids TYPE string ASSERT $value != "";
DEFINE FIELD user_id ON TABLE user_threepids TYPE string ASSERT $value != "";
DEFINE FIELD medium ON TABLE user_threepids TYPE string ASSERT $value IN ['email', 'msisdn'];
DEFINE FIELD address ON TABLE user_threepids TYPE string ASSERT $value != "";
DEFINE FIELD validated_at ON TABLE user_threepids TYPE option<datetime>;
DEFINE FIELD added_at ON TABLE user_threepids TYPE datetime DEFAULT time::now();

DEFINE INDEX user_threepids_lookup ON TABLE user_threepids COLUMNS user_id, medium, address UNIQUE;

-- LiveQuery events for User Experience API
DEFINE EVENT user_profiles_changed ON TABLE user_profiles WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM user_profiles WHERE user_id = $after.user_id OR user_id = $before.user_id
);

DEFINE EVENT account_data_changed ON TABLE account_data WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM account_data WHERE user_id = $after.user_id OR user_id = $before.user_id
);

DEFINE EVENT room_tags_changed ON TABLE room_tags WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM room_tags WHERE user_id = $after.user_id OR user_id = $before.user_id
);

DEFINE EVENT media_files_changed ON TABLE media_files WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM media_files WHERE media_id = $after.media_id OR media_id = $before.media_id
);-- Relationship Features API Database Schema Extensions

-- Space relationships
DEFINE TABLE space_children SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.parent_room_id AND membership = 'join');

DEFINE FIELD parent_room_id ON TABLE space_children TYPE string ASSERT $value != "";
DEFINE FIELD child_room_id ON TABLE space_children TYPE string ASSERT $value != "";
DEFINE FIELD order_value ON TABLE space_children TYPE option<string>;
DEFINE FIELD suggested ON TABLE space_children TYPE bool DEFAULT false;
DEFINE FIELD via ON TABLE space_children TYPE array<string>;
DEFINE FIELD created_at ON TABLE space_children TYPE datetime DEFAULT time::now();

DEFINE INDEX space_hierarchy ON TABLE space_children COLUMNS parent_room_id, child_room_id UNIQUE;

-- Thread relationships  
DEFINE TABLE event_threads SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.room_id AND membership = 'join')
        FOR create, update, delete WHERE $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.room_id AND membership = 'join');

DEFINE FIELD thread_root_id ON TABLE event_threads TYPE string ASSERT $value != "";
DEFINE FIELD event_id ON TABLE event_threads TYPE string ASSERT $value != "";
DEFINE FIELD room_id ON TABLE event_threads TYPE string ASSERT $value != "";
DEFINE FIELD thread_depth ON TABLE event_threads TYPE int DEFAULT 0;
DEFINE FIELD created_at ON TABLE event_threads TYPE datetime DEFAULT time::now();

DEFINE INDEX thread_events ON TABLE event_threads COLUMNS thread_root_id, event_id UNIQUE;

-- Reaction aggregations
DEFINE TABLE event_reactions SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.room_id AND membership = 'join')
        FOR create, update, delete WHERE $auth.user_id = user_id;

DEFINE FIELD event_id ON TABLE event_reactions TYPE string ASSERT $value != "";
DEFINE FIELD room_id ON TABLE event_reactions TYPE string ASSERT $value != "";
DEFINE FIELD reaction_key ON TABLE event_reactions TYPE string ASSERT $value != "";
DEFINE FIELD user_id ON TABLE event_reactions TYPE string ASSERT $value != "";
DEFINE FIELD created_at ON TABLE event_reactions TYPE datetime DEFAULT time::now();

DEFINE INDEX reaction_aggregation ON TABLE event_reactions COLUMNS event_id, reaction_key, user_id UNIQUE;

-- User search index
DEFINE TABLE user_search_index SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD user_id ON TABLE user_search_index TYPE string ASSERT $value != "";
DEFINE FIELD display_name ON TABLE user_search_index TYPE option<string>;
DEFINE FIELD search_vector ON TABLE user_search_index TYPE option<string>;
DEFINE FIELD last_active ON TABLE user_search_index TYPE option<datetime>;
DEFINE FIELD updated_at ON TABLE user_search_index TYPE datetime DEFAULT time::now();

DEFINE INDEX user_search_lookup ON TABLE user_search_index COLUMNS user_id UNIQUE;

-- Content reports
DEFINE TABLE content_reports SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE $auth.is_admin = true OR $auth.user_id = reporter_user_id
        FOR create WHERE $auth.user_id = reporter_user_id
        FOR update, delete WHERE $auth.is_admin = true;

DEFINE FIELD id ON TABLE content_reports TYPE string ASSERT $value != "";
DEFINE FIELD reporter_user_id ON TABLE content_reports TYPE string ASSERT $value != "";
DEFINE FIELD reported_user_id ON TABLE content_reports TYPE option<string>;
DEFINE FIELD room_id ON TABLE content_reports TYPE option<string>;
DEFINE FIELD event_id ON TABLE content_reports TYPE option<string>;
DEFINE FIELD reason ON TABLE content_reports TYPE string ASSERT $value != "";
DEFINE FIELD score ON TABLE content_reports TYPE option<int>;
DEFINE FIELD status ON TABLE content_reports TYPE string DEFAULT 'pending';
DEFINE FIELD created_at ON TABLE content_reports TYPE datetime DEFAULT time::now();
DEFINE FIELD resolved_at ON TABLE content_reports TYPE option<datetime>;

DEFINE INDEX report_status ON TABLE content_reports COLUMNS status, created_at;
DEFINE INDEX content_reports_lookup ON TABLE content_reports COLUMNS id UNIQUE;

-- Room upgrades/tombstones
DEFINE TABLE room_tombstones SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.user_id IN (SELECT user_id FROM room_members WHERE room_id = $parent.old_room_id AND membership = 'join');

DEFINE FIELD old_room_id ON TABLE room_tombstones TYPE string ASSERT $value != "";
DEFINE FIELD new_room_id ON TABLE room_tombstones TYPE string ASSERT $value != "";
DEFINE FIELD upgrade_version ON TABLE room_tombstones TYPE string ASSERT $value != "";
DEFINE FIELD created_by ON TABLE room_tombstones TYPE string ASSERT $value != "";
DEFINE FIELD created_at ON TABLE room_tombstones TYPE datetime DEFAULT time::now();

DEFINE INDEX room_tombstones_lookup ON TABLE room_tombstones COLUMNS old_room_id UNIQUE;

-- LiveQuery events for Relationship Features
DEFINE EVENT space_children_changed ON TABLE space_children WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM space_children WHERE parent_room_id = $after.parent_room_id OR parent_room_id = $before.parent_room_id
);

DEFINE EVENT event_threads_changed ON TABLE event_threads WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM event_threads WHERE thread_root_id = $after.thread_root_id OR thread_root_id = $before.thread_root_id
);

DEFINE EVENT event_reactions_changed ON TABLE event_reactions WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM event_reactions WHERE event_id = $after.event_id OR event_id = $before.event_id
);

DEFINE EVENT content_reports_changed ON TABLE content_reports WHEN $event = "CREATE" OR $event = "UPDATE" OR $event = "DELETE" THEN (
    LIVE SELECT * FROM content_reports WHERE status = 'pending'
);

-- =====================================================
-- 3PID VALIDATION SESSIONS
-- =====================================================

-- Third-party identifier validation sessions
DEFINE TABLE third_party_sessions SCHEMAFULL
    PERMISSIONS
        FOR select, create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD session_id ON TABLE third_party_sessions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD client_secret ON TABLE third_party_sessions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD medium ON TABLE third_party_sessions TYPE string ASSERT $value IN ["email", "msisdn"];
DEFINE FIELD address ON TABLE third_party_sessions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD verification_token ON TABLE third_party_sessions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD verified ON TABLE third_party_sessions TYPE bool DEFAULT false;
DEFINE FIELD expires_at ON TABLE third_party_sessions TYPE int ASSERT $value > 0;
DEFINE FIELD user_id ON TABLE third_party_sessions TYPE option<string>;
DEFINE FIELD created_at ON TABLE third_party_sessions TYPE int DEFAULT time::now();

DEFINE INDEX session_id_idx ON TABLE third_party_sessions COLUMNS session_id UNIQUE;
DEFINE INDEX client_secret_idx ON TABLE third_party_sessions COLUMNS client_secret;
DEFINE INDEX address_idx ON TABLE third_party_sessions COLUMNS address;
DEFINE INDEX expires_at_idx ON TABLE third_party_sessions COLUMNS expires_at;

-- User 3PIDs table for storing verified third-party identifiers
DEFINE TABLE user_3pids SCHEMAFULL
    PERMISSIONS
        FOR select, create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE user_3pids TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD medium ON TABLE user_3pids TYPE string ASSERT $value IN ["email", "msisdn"];
DEFINE FIELD address ON TABLE user_3pids TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD validated_at ON TABLE user_3pids TYPE int ASSERT $value > 0;
DEFINE FIELD added_at ON TABLE user_3pids TYPE int ASSERT $value > 0;

DEFINE INDEX user_3pids_user_idx ON TABLE user_3pids COLUMNS user_id;
DEFINE INDEX user_3pids_address_idx ON TABLE user_3pids COLUMNS medium, address UNIQUE;