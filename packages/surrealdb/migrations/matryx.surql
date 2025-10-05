-- =====================================================
-- Migration: 000
-- Table: access_token
-- Entity: packages/entity/src/types/access_token.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Access token table - For token-based authentication
DEFINE TABLE access_token SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD token ON TABLE access_token TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE access_token TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE access_token TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE access_token TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE access_token TYPE option<datetime>;
DEFINE FIELD last_used_at ON TABLE access_token TYPE option<datetime>;

DEFINE INDEX access_token_token_idx ON TABLE access_token COLUMNS token UNIQUE;
DEFINE INDEX access_token_user_device_idx ON TABLE access_token COLUMNS user_id, device_id;
DEFINE INDEX access_token_expires_idx ON TABLE access_token COLUMNS expires_at;


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


-- =====================================================
-- Migration: 002
-- Table: authentication_content
-- Entity: packages/entity/src/types/authentication_content.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 003
-- Table: backfill_response
-- Entity: packages/entity/src/types/backfill_response.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- BackfillResponse table - matches BackfillResponse entity exactly
DEFINE TABLE backfill_response SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD origin ON TABLE backfill_response TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD origin_server_ts ON TABLE backfill_response TYPE int ASSERT $value > 0;
DEFINE FIELD pdus ON TABLE backfill_response TYPE array<object> DEFAULT [];


-- =====================================================
-- Migration: 004
-- Table: content_reports
-- Entity: packages/entity/src/types/content_reports.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 005
-- Table: cross_signing_key
-- Entity: packages/entity/src/types/cross_signing_key.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- CrossSigningKey table - matches CrossSigningKey entity exactly
DEFINE TABLE cross_signing_key SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Cross-signing keys are public for verification in Matrix
        FOR create, update, delete WHERE $auth.user_id != NONE;  -- Users manage their own cross-signing keys

DEFINE FIELD user_id ON TABLE cross_signing_key TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD usage ON TABLE cross_signing_key TYPE array<string> ASSERT array::len($value) > 0;
DEFINE FIELD keys ON TABLE cross_signing_key TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD signatures ON TABLE cross_signing_key TYPE option<object>;


-- =====================================================
-- Migration: 006
-- Table: cross_signing_keys
-- Entity: packages/entity/src/types/cross_signing_keys.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Cross-signing keys table - E2E encryption cross-signing keys (queried by keys.rs)
DEFINE TABLE cross_signing_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Cross-signing keys are public for verification
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE cross_signing_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_type ON TABLE cross_signing_keys TYPE string ASSERT $value IN ['master', 'self_signing', 'user_signing'];
DEFINE FIELD key_id ON TABLE cross_signing_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_data ON TABLE cross_signing_keys TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD usage ON TABLE cross_signing_keys TYPE array<string> DEFAULT [];
DEFINE FIELD signatures ON TABLE cross_signing_keys TYPE option<object>;
DEFINE FIELD created_at ON TABLE cross_signing_keys TYPE datetime DEFAULT time::now();


-- =====================================================
-- Migration: 007
-- Table: device
-- Entity: packages/entity/src/types/device.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Device table - Matrix spec compliant (spec/client/04_security_encryption.md:247-250)
DEFINE TABLE device SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Devices are accessible to authenticated users managing them
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD device_id ON TABLE device TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE device TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD display_name ON TABLE device TYPE option<string>;
DEFINE FIELD last_seen_ip ON TABLE device TYPE option<string>;
DEFINE FIELD last_seen_ts ON TABLE device TYPE option<int>;

DEFINE INDEX device_id_idx ON TABLE device COLUMNS device_id UNIQUE;
DEFINE INDEX device_user_idx ON TABLE device COLUMNS user_id;
DEFINE INDEX device_user_device_idx ON TABLE device COLUMNS user_id, device_id UNIQUE;


-- =====================================================
-- Migration: 008
-- Table: device_key
-- Entity: packages/entity/src/types/device_key.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 009
-- Table: device_keys
-- Entity: packages/entity/src/types/device_keys.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Device keys table - E2E encryption device keys (queried by keys.rs)
DEFINE TABLE device_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE  -- Any authenticated user (keys are discoverable for crypto)
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE device_keys TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD device_id ON TABLE device_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithms ON TABLE device_keys TYPE array<string> DEFAULT [];
DEFINE FIELD keys ON TABLE device_keys TYPE object DEFAULT {};
DEFINE FIELD signatures ON TABLE device_keys TYPE object DEFAULT {};
DEFINE FIELD unsigned ON TABLE device_keys TYPE option<object>;
DEFINE FIELD user_signing_key ON TABLE device_keys TYPE option<string>;
DEFINE FIELD device_display_name ON TABLE device_keys TYPE option<string>;

DEFINE INDEX device_keys_user_device_idx ON TABLE device_keys COLUMNS user_id, device_id UNIQUE;
DEFINE INDEX device_keys_user_idx ON TABLE device_keys COLUMNS user_id;


-- =====================================================
-- Migration: 010
-- Table: encrypted_file
-- Entity: packages/entity/src/types/encrypted_file.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

DEFINE TABLE encrypted_file SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.user_id != NONE;  -- Files are accessible to authenticated users

DEFINE FIELD url ON TABLE encrypted_file TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key ON TABLE encrypted_file TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD iv ON TABLE encrypted_file TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD hashes ON TABLE encrypted_file TYPE object DEFAULT {};
DEFINE FIELD v ON TABLE encrypted_file TYPE string ASSERT string::is::not::empty($value);


-- =====================================================
-- Migration: 011
-- Table: event
-- Entity: packages/entity/src/types/event.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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
DEFINE INDEX event_room_ts_idx ON TABLE event COLUMNS room_id, origin_server_ts;
DEFINE INDEX event_sender_idx ON TABLE event COLUMNS sender;


-- =====================================================
-- Migration: 012
-- Table: event_reactions
-- Entity: packages/entity/src/types/event_reactions.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 013
-- Table: event_relations
-- Entity: packages/entity/src/types/event_relations.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Event relations table - Event relationships (replies, edits, reactions) (queried by event_replacement.rs)
DEFINE TABLE event_relations SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD event_id ON TABLE event_relations TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD relates_to_event_id ON TABLE event_relations TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD rel_type ON TABLE event_relations TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE event_relations TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE event_relations TYPE datetime DEFAULT time::now();


-- =====================================================
-- Migration: 014
-- Table: event_template
-- Entity: packages/entity/src/types/event_template.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 015
-- Table: event_threads
-- Entity: packages/entity/src/types/event_threads.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 016
-- Table: fallback_keys
-- Entity: packages/entity/src/types/fallback_keys.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Fallback keys table - E2E encryption fallback keys (queried by keys.rs)
DEFINE TABLE fallback_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE fallback_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE fallback_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_id ON TABLE fallback_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_data ON TABLE fallback_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm_type ON TABLE fallback_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD signatures ON TABLE fallback_keys TYPE option<object>;
DEFINE FIELD created_at ON TABLE fallback_keys TYPE datetime DEFAULT time::now();
DEFINE FIELD used ON TABLE fallback_keys TYPE bool DEFAULT false;


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


-- =====================================================
-- Migration: 019
-- Table: filter
-- Entity: packages/entity/src/types/filter.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Filter table - Matrix filtering criteria (matches Filter entity exactly)
DEFINE TABLE filter SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.user_id != NONE;  -- Filters are user-specific resources

DEFINE FIELD limit ON TABLE filter TYPE option<int>;
DEFINE FIELD types ON TABLE filter TYPE option<array<string>>;
DEFINE FIELD not_types ON TABLE filter TYPE option<array<string>>;
DEFINE FIELD senders ON TABLE filter TYPE option<array<string>>;
DEFINE FIELD not_senders ON TABLE filter TYPE option<array<string>>;


-- =====================================================
-- Migration: 020
-- Table: invite_v1_request
-- Entity: packages/entity/src/types/invite_v1_request.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 021
-- Table: invite_v2_request
-- Entity: packages/entity/src/types/invite_v2_request.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- =====================================================
-- BATCH 2: FEDERATION ENTITIES
-- =====================================================-- InviteV2Request table - matches InviteV2Request entity exactly
DEFINE TABLE invite_v2_request SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD event ON TABLE invite_v2_request TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD invite_room_state ON TABLE invite_v2_request TYPE option<array<object>>;
DEFINE FIELD room_version ON TABLE invite_v2_request TYPE string ASSERT string::is::not::empty($value);


-- =====================================================
-- Migration: 022
-- Table: join_room_response
-- Entity: packages/entity/src/types/join_room_response.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- JoinRoomResponse table - matches JoinRoomResponse entity exactly
DEFINE TABLE join_room_response SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD room_id ON TABLE join_room_response TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE join_room_response TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD timestamp ON TABLE join_room_response TYPE datetime DEFAULT time::now();


-- =====================================================
-- Migration: 023
-- Table: key_claim_request
-- Entity: packages/entity/src/types/key_claim_request.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- KeyClaimRequest table - matches KeyClaimRequest entity exactly
DEFINE TABLE key_claim_request SCHEMAFULL
    PERMISSIONS
        FOR select, create WHERE $auth.user_id != NONE
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD one_time_keys ON TABLE key_claim_request TYPE object DEFAULT {};
DEFINE FIELD requesting_user_id ON TABLE key_claim_request TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD timestamp ON TABLE key_claim_request TYPE datetime DEFAULT time::now();


-- =====================================================
-- Migration: 024
-- Table: make_join_response
-- Entity: packages/entity/src/types/make_join_response.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- MakeJoinResponse table - matches MakeJoinResponse entity exactly
DEFINE TABLE make_join_response SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD event ON TABLE make_join_response TYPE object ASSERT object::len($value) > 0;
DEFINE FIELD room_version ON TABLE make_join_response TYPE string ASSERT string::is::not::empty($value);


-- =====================================================
-- Migration: 025
-- Table: matrix_federation_event
-- Entity: packages/entity/src/types/matrix_federation_event.rs
-- Repositories: [To be filled during repository analysis]
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


-- =====================================================
-- Migration: 026
-- Table: matrix_global_sequence
-- Entity: packages/entity/src/types/matrix_global_sequence.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Global sequence number for Matrix sync ordering
DEFINE TABLE matrix_global_sequence SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true
        FOR create, update, delete WHERE $auth.admin = true;
DEFINE FIELD current_sequence ON TABLE matrix_global_sequence TYPE int DEFAULT 0;


-- =====================================================
-- Migration: 027
-- Table: matrix_sync_account_data
-- Entity: packages/entity/src/types/matrix_sync_account_data.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 028
-- Table: matrix_sync_batch
-- Entity: packages/entity/src/types/matrix_sync_batch.rs
-- Repositories: [To be filled during repository analysis]
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


-- =====================================================
-- Migration: 029
-- Table: matrix_sync_device_list
-- Entity: packages/entity/src/types/matrix_sync_device_list.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 030
-- Table: matrix_sync_presence
-- Entity: packages/entity/src/types/matrix_sync_presence.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 031
-- Table: matrix_sync_retention
-- Entity: packages/entity/src/types/matrix_sync_retention.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Matrix sync retention configuration
DEFINE TABLE matrix_sync_retention SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.admin = true;

DEFINE FIELD table_name ON TABLE matrix_sync_retention TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD retention_days ON TABLE matrix_sync_retention TYPE int ASSERT $value > 0;
DEFINE FIELD cleanup_batch_size ON TABLE matrix_sync_retention TYPE int DEFAULT 1000;
DEFINE FIELD last_cleanup ON TABLE matrix_sync_retention TYPE datetime DEFAULT time::now();


-- =====================================================
-- Migration: 032
-- Table: matrix_sync_room_event
-- Entity: packages/entity/src/types/matrix_sync_room_event.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 033
-- Table: matrix_sync_to_device
-- Entity: packages/entity/src/types/matrix_sync_to_device.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 034
-- Table: media_files
-- Entity: packages/entity/src/types/media_files.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 035
-- Table: membership
-- Entity: packages/entity/src/types/membership.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Membership table - Room membership tracking (matches Membership entity)
DEFINE TABLE membership SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE membership TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD user_id ON TABLE membership TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD membership ON TABLE membership TYPE string ASSERT $value IN ['invite', 'join', 'leave', 'ban', 'knock'];
DEFINE FIELD display_name ON TABLE membership TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE membership TYPE option<string>;
DEFINE FIELD reason ON TABLE membership TYPE option<string>;
DEFINE FIELD invited_by ON TABLE membership TYPE option<string>;
DEFINE FIELD updated_at ON TABLE membership TYPE datetime DEFAULT time::now();

DEFINE INDEX membership_room_user_idx ON TABLE membership COLUMNS room_id, user_id UNIQUE;
DEFINE INDEX membership_room_idx ON TABLE membership COLUMNS room_id;
DEFINE INDEX membership_user_idx ON TABLE membership COLUMNS user_id;
DEFINE INDEX membership_room_state_idx ON TABLE membership COLUMNS room_id, membership;
DEFINE INDEX membership_user_state_idx ON TABLE membership COLUMNS user_id, membership;
DEFINE INDEX membership_state_idx ON TABLE membership COLUMNS membership;


-- =====================================================
-- Migration: 036
-- Table: membership_event_content
-- Entity: packages/entity/src/types/membership_event_content.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 037
-- Table: one_time_key_object
-- Entity: packages/entity/src/types/one_time_key_object.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 038
-- Table: one_time_keys
-- Entity: packages/entity/src/types/one_time_keys.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- One-time keys table - E2E encryption one-time keys (queried by keys.rs)
DEFINE TABLE one_time_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE one_time_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE one_time_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_id ON TABLE one_time_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_data ON TABLE one_time_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm_type ON TABLE one_time_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD signatures ON TABLE one_time_keys TYPE option<object>;
DEFINE FIELD claimed ON TABLE one_time_keys TYPE bool DEFAULT false;
DEFINE FIELD claimed_at ON TABLE one_time_keys TYPE option<datetime>;
DEFINE FIELD claimed_by ON TABLE one_time_keys TYPE option<string>;
DEFINE FIELD created_at ON TABLE one_time_keys TYPE datetime DEFAULT time::now();

DEFINE INDEX one_time_keys_user_device_claimed_idx ON TABLE one_time_keys COLUMNS user_id, device_id, claimed;
DEFINE INDEX one_time_keys_user_device_key_idx ON TABLE one_time_keys COLUMNS user_id, device_id, key_id UNIQUE;


-- =====================================================
-- Migration: 039
-- Table: pdu
-- Entity: packages/entity/src/types/pdu.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 040
-- Table: power_levels
-- Entity: packages/entity/src/types/power_levels.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Power levels table - Room power levels (queried by membership.rs)
DEFINE TABLE power_levels SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE power_levels TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE power_levels TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD power_level ON TABLE power_levels TYPE int DEFAULT 0;
DEFINE FIELD updated_at ON TABLE power_levels TYPE datetime DEFAULT time::now();


-- =====================================================
-- Migration: 041
-- Table: public_rooms
-- Entity: packages/entity/src/types/public_rooms.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 042
-- Table: published_rooms_chunk
-- Entity: packages/entity/src/types/published_rooms_chunk.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 043
-- Table: push_rule
-- Entity: packages/entity/src/types/push_rule.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 044
-- Table: push_rules
-- Entity: packages/entity/src/types/push_rules.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 045
-- Table: pushers
-- Entity: packages/entity/src/types/pushers.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 046
-- Table: refresh_tokens
-- Entity: packages/entity/src/types/refresh_tokens.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Refresh token table - For token refresh
DEFINE TABLE refresh_tokens SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.user_id != NONE
        FOR update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD id ON TABLE refresh_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD token ON TABLE refresh_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE refresh_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD device_id ON TABLE refresh_tokens TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE refresh_tokens TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE refresh_tokens TYPE datetime;
DEFINE FIELD used ON TABLE refresh_tokens TYPE bool DEFAULT false;
DEFINE FIELD revoked ON TABLE refresh_tokens TYPE bool DEFAULT false;
DEFINE FIELD is_revoked ON TABLE refresh_tokens TYPE bool DEFAULT false;
DEFINE FIELD revoked_at ON TABLE refresh_tokens TYPE option<datetime>;
DEFINE FIELD updated_at ON TABLE refresh_tokens TYPE datetime DEFAULT time::now();

DEFINE INDEX refresh_token_token_idx ON TABLE refresh_tokens COLUMNS token UNIQUE;
DEFINE INDEX refresh_token_user_idx ON TABLE refresh_tokens COLUMNS user_id;
DEFINE INDEX refresh_token_expires_idx ON TABLE refresh_tokens COLUMNS expires_at;
DEFINE INDEX refresh_token_revoked_idx ON TABLE refresh_tokens COLUMNS is_revoked;


-- =====================================================
-- Migration: 047
-- Table: room
-- Entity: packages/entity/src/types/room.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room table - Core Matrix room entity (matches Room entity from packages/entity/src/types/room.rs)
DEFINE TABLE room SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update WHERE creator = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE room TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '!') AND string::contains($value, ':');
DEFINE FIELD room_version ON TABLE room TYPE string DEFAULT '10';
DEFINE FIELD creator ON TABLE room TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD created_at ON TABLE room TYPE datetime DEFAULT time::now();
DEFINE FIELD name ON TABLE room TYPE option<string>;
DEFINE FIELD topic ON TABLE room TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE room TYPE option<string>;
DEFINE FIELD canonical_alias ON TABLE room TYPE option<string>;
DEFINE FIELD alt_aliases ON TABLE room TYPE option<array<string>>;
DEFINE FIELD join_rule ON TABLE room TYPE option<string>;
DEFINE FIELD history_visibility ON TABLE room TYPE option<string>;
DEFINE FIELD guest_access ON TABLE room TYPE option<string>;
DEFINE FIELD power_levels ON TABLE room TYPE option<object>;
DEFINE FIELD encryption ON TABLE room TYPE option<object>;
DEFINE FIELD federate ON TABLE room TYPE option<bool> DEFAULT true;
DEFINE FIELD room_type ON TABLE room TYPE option<string>;
DEFINE FIELD is_public ON TABLE room TYPE option<bool> DEFAULT false;
DEFINE FIELD is_direct ON TABLE room TYPE option<bool> DEFAULT false;
DEFINE FIELD join_rules ON TABLE room TYPE option<string>;
DEFINE FIELD tombstone ON TABLE room TYPE option<object>;
DEFINE FIELD predecessor ON TABLE room TYPE option<object>;
DEFINE FIELD state_events_count ON TABLE room TYPE option<int> DEFAULT 0;
DEFINE FIELD updated_at ON TABLE room TYPE option<datetime>;
DEFINE FIELD visibility ON TABLE room TYPE string DEFAULT 'private';

DEFINE INDEX room_id_idx ON TABLE room COLUMNS room_id UNIQUE;
DEFINE INDEX room_creator_idx ON TABLE room COLUMNS creator;
DEFINE INDEX room_visibility_idx ON TABLE room COLUMNS visibility;
DEFINE INDEX room_is_public_idx ON TABLE room COLUMNS is_public;
DEFINE INDEX room_type_idx ON TABLE room COLUMNS room_type;


-- =====================================================
-- Migration: 048
-- Table: room_alias_mapping
-- Entity: packages/entity/src/types/room_alias_mapping.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- RoomAliasMapping table - matches RoomAliasMapping entity exactly
DEFINE TABLE room_alias_mapping SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Room aliases are publicly discoverable
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE room_alias_mapping TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD alias ON TABLE room_alias_mapping TYPE string ASSERT string::is::not::empty($value);


-- =====================================================
-- Migration: 049
-- Table: room_key_backup
-- Entity: packages/entity/src/types/room_key_backup.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

DEFINE TABLE room_key_backup SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD version ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD auth_data ON TABLE room_key_backup TYPE object DEFAULT {};
DEFINE FIELD count ON TABLE room_key_backup TYPE int DEFAULT 0;
DEFINE FIELD etag ON TABLE room_key_backup TYPE string ASSERT string::is::not::empty($value);


-- =====================================================
-- Migration: 050
-- Table: room_key_backup_versions
-- Entity: packages/entity/src/types/room_key_backup_versions.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room key backup versions table - E2E encryption room key backup versions (queried by room_keys.rs)
DEFINE TABLE room_key_backup_versions SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE room_key_backup_versions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD version ON TABLE room_key_backup_versions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm ON TABLE room_key_backup_versions TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD auth_data ON TABLE room_key_backup_versions TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE room_key_backup_versions TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE room_key_backup_versions TYPE datetime DEFAULT time::now();
DEFINE FIELD deleted ON TABLE room_key_backup_versions TYPE bool DEFAULT false;


-- =====================================================
-- Migration: 051
-- Table: room_key_backups
-- Entity: packages/entity/src/types/room_key_backups.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room key backups table - E2E encryption room key backups (queried by room_keys.rs, key_backup.rs)
DEFINE TABLE room_key_backups SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE room_key_backups TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD version ON TABLE room_key_backups TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE room_key_backups TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD session_id ON TABLE room_key_backups TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD first_message_index ON TABLE room_key_backups TYPE int;
DEFINE FIELD forwarded_count ON TABLE room_key_backups TYPE int;
DEFINE FIELD is_verified ON TABLE room_key_backups TYPE bool;
DEFINE FIELD session_data ON TABLE room_key_backups TYPE object DEFAULT {};
DEFINE FIELD created_at ON TABLE room_key_backups TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE room_key_backups TYPE datetime DEFAULT time::now();
DEFINE FIELD backup_version ON TABLE room_key_backups TYPE string;


-- =====================================================
-- Migration: 052
-- Table: room_members
-- Entity: packages/entity/src/types/room_members.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room members table - Alternative membership tracking (queried by multiple repositories)
DEFINE TABLE room_members SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE room_members TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE room_members TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD membership ON TABLE room_members TYPE string ASSERT $value IN ['invite', 'join', 'leave', 'ban', 'knock'];
DEFINE FIELD display_name ON TABLE room_members TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE room_members TYPE option<string>;
DEFINE FIELD reason ON TABLE room_members TYPE option<string>;
DEFINE FIELD invited_by ON TABLE room_members TYPE option<string>;
DEFINE FIELD joined_at ON TABLE room_members TYPE option<datetime>;
DEFINE FIELD updated_at ON TABLE room_members TYPE datetime DEFAULT time::now();


-- =====================================================
-- Migration: 053
-- Table: room_membership
-- Entity: packages/entity/src/types/room_membership.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 054
-- Table: room_memberships
-- Entity: packages/entity/src/types/room_memberships.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Room memberships table - Alternative name for membership cache (queried by auth.rs, user.rs)
DEFINE TABLE room_memberships SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create, update WHERE $auth.user_id != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE room_memberships TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE room_memberships TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD membership ON TABLE room_memberships TYPE string ASSERT $value IN ['invite', 'join', 'leave', 'ban', 'knock'];
DEFINE FIELD display_name ON TABLE room_memberships TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE room_memberships TYPE option<string>;
DEFINE FIELD updated_at ON TABLE room_memberships TYPE datetime DEFAULT time::now();


-- =====================================================
-- Migration: 055
-- Table: room_state_response
-- Entity: packages/entity/src/types/room_state_response.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

DEFINE TABLE room_state_response SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD auth_chain ON TABLE room_state_response TYPE array<object> DEFAULT [];
DEFINE FIELD pdus ON TABLE room_state_response TYPE array<object> DEFAULT [];
DEFINE FIELD room_id ON TABLE room_state_response TYPE string ASSERT string::is::not::empty($value);


-- =====================================================
-- Migration: 056
-- Table: room_tags
-- Entity: packages/entity/src/types/room_tags.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 057
-- Table: room_tombstones
-- Entity: packages/entity/src/types/room_tombstones.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 058
-- Table: rooms
-- Entity: packages/entity/src/types/rooms.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Rooms table - Alternative room storage (queried by membership.rs and other repositories)
DEFINE TABLE rooms SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.user_id != NONE
        FOR create WHERE $auth.user_id != NONE
        FOR update WHERE creator = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE rooms TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD creator ON TABLE rooms TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE rooms TYPE datetime DEFAULT time::now();
DEFINE FIELD room_version ON TABLE rooms TYPE string DEFAULT '10';
DEFINE FIELD name ON TABLE rooms TYPE option<string>;
DEFINE FIELD topic ON TABLE rooms TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE rooms TYPE option<string>;
DEFINE FIELD visibility ON TABLE rooms TYPE string DEFAULT 'private';


-- =====================================================
-- Migration: 059
-- Table: search_index
-- Entity: packages/entity/src/types/search_index.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 060
-- Table: send_join_request
-- Entity: packages/entity/src/types/send_join_request.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 061
-- Table: send_join_room_state
-- Entity: packages/entity/src/types/send_join_room_state.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- SendJoinRoomState table - matches SendJoinRoomState entity exactly
DEFINE TABLE send_join_room_state SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.server_name != NONE;

DEFINE FIELD state ON TABLE send_join_room_state TYPE array<object> DEFAULT [];
DEFINE FIELD auth_chain ON TABLE send_join_room_state TYPE array<object> DEFAULT [];


-- =====================================================
-- Migration: 062
-- Table: server_config
-- Entity: packages/entity/src/types/server_config.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 063
-- Table: server_info
-- Entity: packages/entity/src/types/server_info.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- ServerInfo table - matches ServerInfo entity exactly
DEFINE TABLE server_info SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Server info is publicly accessible
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD server ON TABLE server_info TYPE object ASSERT object::len($value) > 0;


-- =====================================================
-- Migration: 064
-- Table: server_signing_key
-- Entity: packages/entity/src/types/server_signing_key.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Server signing key table - Individual server signing key (queried by session.rs)
DEFINE TABLE server_signing_key SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true -- Server keys are public for verification
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD key_id ON TABLE server_signing_key TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_data ON TABLE server_signing_key TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm ON TABLE server_signing_key TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE server_signing_key TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE server_signing_key TYPE option<datetime>;
DEFINE FIELD is_active ON TABLE server_signing_key TYPE bool DEFAULT true;


-- =====================================================
-- Migration: 065
-- Table: server_signing_keys
-- Entity: packages/entity/src/types/server_signing_keys.rs
-- Repositories: [To be filled during repository analysis]
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


-- =====================================================
-- Migration: 066
-- Table: session
-- Entity: packages/entity/src/types/session.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Session table - Matrix session management (matches Session entity from packages/entity/src/types/session.rs)
DEFINE TABLE session SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.user_id != NONE
        FOR update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD session_id ON TABLE session TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE session TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD device_id ON TABLE session TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD access_token ON TABLE session TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD refresh_token ON TABLE session TYPE option<string>;
DEFINE FIELD created_at ON TABLE session TYPE datetime DEFAULT time::now();
DEFINE FIELD expires_at ON TABLE session TYPE option<datetime>;
DEFINE FIELD last_seen ON TABLE session TYPE option<datetime>;
DEFINE FIELD last_used_at ON TABLE session TYPE option<datetime>;
DEFINE FIELD last_used_ip ON TABLE session TYPE option<string>;
DEFINE FIELD user_agent ON TABLE session TYPE option<string>;
DEFINE FIELD is_active ON TABLE session TYPE bool DEFAULT true;
DEFINE FIELD valid ON TABLE session TYPE bool DEFAULT true;
DEFINE FIELD puppets_user_id ON TABLE session TYPE option<string>;
DEFINE FIELD updated_at ON TABLE session TYPE datetime DEFAULT time::now();

DEFINE INDEX session_id_idx ON TABLE session COLUMNS session_id UNIQUE;
DEFINE INDEX session_access_token_idx ON TABLE session COLUMNS access_token UNIQUE;
DEFINE INDEX session_user_idx ON TABLE session COLUMNS user_id;
DEFINE INDEX session_user_active_idx ON TABLE session COLUMNS user_id, is_active;
DEFINE INDEX session_device_idx ON TABLE session COLUMNS device_id;


-- =====================================================
-- Migration: 067
-- Table: session_data
-- Entity: packages/entity/src/types/session_data.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- SessionData table - matches SessionData entity exactly
DEFINE TABLE session_data SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE session_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD session_id ON TABLE session_data TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD encrypted_data ON TABLE session_data TYPE object DEFAULT {};


-- =====================================================
-- Migration: 068
-- Table: space_children
-- Entity: packages/entity/src/types/space_children.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 069
-- Table: third_party_invite
-- Entity: packages/entity/src/types/third_party_invite.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 070
-- Table: third_party_sessions
-- Entity: packages/entity/src/types/third_party_sessions.rs
-- Repositories: [To be filled during repository analysis]
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


-- =====================================================
-- Migration: 071
-- Table: thirdparty_locations
-- Entity: packages/entity/src/types/thirdparty_locations.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Third-party locations (rooms/channels)
DEFINE TABLE thirdparty_locations SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD protocol ON TABLE thirdparty_locations TYPE string;
DEFINE FIELD alias ON TABLE thirdparty_locations TYPE string;
DEFINE FIELD fields ON TABLE thirdparty_locations TYPE object;
DEFINE FIELD created_at ON TABLE thirdparty_locations TYPE datetime DEFAULT time::now();


-- =====================================================
-- Migration: 072
-- Table: thirdparty_protocols
-- Entity: packages/entity/src/types/thirdparty_protocols.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 073
-- Table: thirdparty_users
-- Entity: packages/entity/src/types/thirdparty_users.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Third-party users
DEFINE TABLE thirdparty_users SCHEMAFULL
    PERMISSIONS 
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.is_admin = true;

DEFINE FIELD protocol ON TABLE thirdparty_users TYPE string;
DEFINE FIELD userid ON TABLE thirdparty_users TYPE string;
DEFINE FIELD fields ON TABLE thirdparty_users TYPE object;
DEFINE FIELD created_at ON TABLE thirdparty_users TYPE datetime DEFAULT time::now();


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


-- =====================================================
-- Migration: 076
-- Table: typing_notification
-- Entity: packages/entity/src/types/typing_notification.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 077
-- Table: user
-- Entity: packages/entity/src/types/user.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- User table - Core Matrix user entity (matches User entity from packages/entity/src/types/user.rs)
DEFINE TABLE user SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.admin = true
        FOR update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE user TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD display_name ON TABLE user TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE user TYPE option<string>;
DEFINE FIELD password_hash ON TABLE user TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE user TYPE datetime DEFAULT time::now();
DEFINE FIELD last_seen ON TABLE user TYPE option<datetime>;
DEFINE FIELD is_active ON TABLE user TYPE bool DEFAULT true;
DEFINE FIELD is_admin ON TABLE user TYPE bool DEFAULT false;
DEFINE FIELD account_data ON TABLE user TYPE option<object>;

DEFINE INDEX user_id_idx ON TABLE user COLUMNS user_id UNIQUE;
DEFINE INDEX user_active_idx ON TABLE user COLUMNS is_active;
DEFINE INDEX user_admin_idx ON TABLE user COLUMNS is_admin;


-- =====================================================
-- Migration: 078
-- Table: user_3pids
-- Entity: packages/entity/src/types/user_3pids.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 079
-- Table: user_access_tokens
-- Entity: packages/entity/src/types/user_access_tokens.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 080
-- Table: user_account_data
-- Entity: packages/entity/src/types/user_account_data.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 081
-- Table: user_blocks
-- Entity: packages/entity/src/types/user_blocks.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- User blocks table - User blocking functionality (queried by to_device.rs)
DEFINE TABLE user_blocks SCHEMAFULL
    PERMISSIONS
        FOR select WHERE blocker_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE blocker_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD id ON TABLE user_blocks TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD blocker_id ON TABLE user_blocks TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD blocked_id ON TABLE user_blocks TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE user_blocks TYPE datetime DEFAULT time::now();


-- =====================================================
-- Migration: 082
-- Table: user_presence_update
-- Entity: packages/entity/src/types/user_presence_update.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 083
-- Table: user_profiles
-- Entity: packages/entity/src/types/user_profiles.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 084
-- Table: user_reports
-- Entity: packages/entity/src/types/user_reports.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- User reports table - Content moderation reports (queried by reports.rs)
DEFINE TABLE user_reports SCHEMAFULL
    PERMISSIONS
        FOR select WHERE reporter_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE reporter_id = $auth.user_id
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD report_id ON TABLE user_reports TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD reporter_id ON TABLE user_reports TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD reported_user_id ON TABLE user_reports TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE user_reports TYPE option<string>;
DEFINE FIELD event_id ON TABLE user_reports TYPE option<string>;
DEFINE FIELD reason ON TABLE user_reports TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD score ON TABLE user_reports TYPE option<int>;
DEFINE FIELD status ON TABLE user_reports TYPE string DEFAULT 'pending' ASSERT $value IN ['pending', 'investigating', 'escalated', 'resolved', 'dismissed'];
DEFINE FIELD created_at ON TABLE user_reports TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE user_reports TYPE datetime DEFAULT time::now();


-- =====================================================
-- Migration: 085
-- Table: user_search_index
-- Entity: packages/entity/src/types/user_search_index.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 086
-- Table: user_threepids
-- Entity: packages/entity/src/types/user_threepids.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 087
-- Table: users
-- Entity: packages/entity/src/types/users.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Users table - Alternative user table name (queried by user.rs)
DEFINE TABLE users SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create WHERE $auth.admin = true
        FOR update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE users TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD display_name ON TABLE users TYPE option<string>;
DEFINE FIELD avatar_url ON TABLE users TYPE option<string>;
DEFINE FIELD password_hash ON TABLE users TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE users TYPE datetime DEFAULT time::now();
DEFINE FIELD is_active ON TABLE users TYPE bool DEFAULT true;
DEFINE FIELD is_admin ON TABLE users TYPE bool DEFAULT false;


-- =====================================================
-- Migration: 088
-- Table: verification_start
-- Entity: packages/entity/src/types/verification_start.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

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


-- =====================================================
-- Migration: 090
-- Table: backup_version
-- Entity: Based on BackupVersionRecord struct
-- Repositories: key_backup.rs
-- =====================================================

-- E2E encryption key backup version metadata
DEFINE TABLE backup_version SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE backup_version TYPE string ASSERT string::is::not::empty($value);

DEFINE FIELD user_id ON TABLE backup_version TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD version ON TABLE backup_version TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD algorithm ON TABLE backup_version TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD auth_data ON TABLE backup_version TYPE object DEFAULT {};
DEFINE FIELD count ON TABLE backup_version TYPE int DEFAULT 0;
DEFINE FIELD etag ON TABLE backup_version TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD created_at ON TABLE backup_version TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE backup_version TYPE datetime DEFAULT time::now();

DEFINE INDEX backup_version_user_idx ON TABLE backup_version 
    COLUMNS user_id;
DEFINE INDEX backup_version_user_version_idx ON TABLE backup_version 
    COLUMNS user_id, version UNIQUE;
DEFINE INDEX backup_version_created_idx ON TABLE backup_version 
    COLUMNS created_at;


-- =====================================================
-- Migration: 091
-- Table: server_keys
-- Entity: packages/entity/src/types/server_keys_response.rs
-- Repositories: key_server.rs, federation.rs
-- =====================================================

-- Cache federation server signing keys
DEFINE TABLE server_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update WHERE $auth.server_name != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD server_name ON TABLE server_keys TYPE string 
    ASSERT string::is::not::empty($value) 
    AND (string::contains($value, ':') OR string::contains($value, '.'));

DEFINE FIELD verify_keys ON TABLE server_keys TYPE object ASSERT $value != NONE;
DEFINE FIELD old_verify_keys ON TABLE server_keys TYPE option<object>;
DEFINE FIELD valid_until_ts ON TABLE server_keys TYPE int ASSERT $value > 0;
DEFINE FIELD signatures ON TABLE server_keys TYPE object DEFAULT {};
DEFINE FIELD cached_at ON TABLE server_keys TYPE datetime DEFAULT time::now();
DEFINE FIELD key_ids ON TABLE server_keys TYPE option<array<string>>;

DEFINE INDEX server_keys_server_idx ON TABLE server_keys 
    COLUMNS server_name;
DEFINE INDEX server_keys_valid_idx ON TABLE server_keys 
    COLUMNS valid_until_ts;
DEFINE INDEX server_keys_server_valid_idx ON TABLE server_keys 
    COLUMNS server_name, valid_until_ts;


-- =====================================================
-- Migration: 092
-- Table: old_verify_keys
-- Entity: packages/entity/src/types/old_verify_key.rs
-- Repositories: key_server.rs
-- =====================================================

-- Store expired server signing keys for federation
DEFINE TABLE old_verify_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update WHERE $auth.server_name != NONE
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD server_name ON TABLE old_verify_keys TYPE string 
    ASSERT string::is::not::empty($value) 
    AND (string::contains($value, ':') OR string::contains($value, '.'));

DEFINE FIELD key_id ON TABLE old_verify_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key ON TABLE old_verify_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD expired_ts ON TABLE old_verify_keys TYPE int ASSERT $value > 0;
DEFINE FIELD created_at ON TABLE old_verify_keys TYPE datetime DEFAULT time::now();

DEFINE INDEX old_verify_keys_server_key_idx ON TABLE old_verify_keys 
    COLUMNS server_name, key_id UNIQUE;
DEFINE INDEX old_verify_keys_expired_idx ON TABLE old_verify_keys 
    COLUMNS expired_ts;
DEFINE INDEX old_verify_keys_server_idx ON TABLE old_verify_keys 
    COLUMNS server_name;


-- =====================================================
-- Migration: 093
-- Table: backup_keys
-- Entity: Based on RoomKeyBackupRecord struct
-- Repositories: key_backup.rs
-- =====================================================

-- Store encrypted room keys in user's E2E key backup
DEFINE TABLE backup_keys SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD id ON TABLE backup_keys TYPE string ASSERT string::is::not::empty($value);

DEFINE FIELD user_id ON TABLE backup_keys TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD version ON TABLE backup_keys TYPE string ASSERT string::is::not::empty($value);

DEFINE FIELD room_id ON TABLE backup_keys TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '!') 
    AND string::contains($value, ':');

DEFINE FIELD session_id ON TABLE backup_keys TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD key_data ON TABLE backup_keys TYPE object ASSERT $value != NONE;
DEFINE FIELD created_at ON TABLE backup_keys TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON TABLE backup_keys TYPE datetime DEFAULT time::now();

DEFINE INDEX backup_keys_user_version_idx ON TABLE backup_keys 
    COLUMNS user_id, version;
DEFINE INDEX backup_keys_user_version_room_idx ON TABLE backup_keys 
    COLUMNS user_id, version, room_id;
DEFINE INDEX backup_keys_user_version_room_session_idx ON TABLE backup_keys 
    COLUMNS user_id, version, room_id, session_id UNIQUE;


-- =====================================================
-- Migration: 095
-- LiveQuery Event Infrastructure
-- Purpose: DEFINE EVENT statements for real-time sync
-- =====================================================

-- Global sequence function for ordering sync events
DEFINE FUNCTION fn::next_sequence() {
    -- Get current sequence number
    LET $current = (
        SELECT VALUE sequence_number
        FROM matrix_global_sequence:global
        LIMIT 1
    );

    -- Increment and return new sequence
    LET $next = $current + 1;

    UPDATE matrix_global_sequence:global
    SET sequence_number = $next,
        updated_at = time::now();

    RETURN $next;
};

-- PDU events for room timeline
DEFINE EVENT pdu_sync_event ON TABLE pdu WHEN $event = "CREATE" THEN (
    LET $sequence = fn::next_sequence();

    -- Find room members to notify
    LET $room_members = (
        SELECT user_id FROM room_membership
        WHERE room_id = $this.room_id
        AND membership = 'join'
    );

    -- Create sync records for each member
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
                sender: $this.sender,
                content: $this.content,
                origin_server_ts: $this.origin_server_ts,
                unsigned: $this.unsigned
            },
            state_event = {},
            created_at = time::now()
    }
);

-- State events for room state
DEFINE EVENT state_sync_event ON TABLE event
WHEN $event = "CREATE" AND $this.state_key != NONE THEN (
    LET $sequence = fn::next_sequence();

    -- Find room members to notify
    LET $room_members = (
        SELECT user_id FROM room_membership
        WHERE room_id = $this.room_id
        AND membership = 'join'
    );

    -- Create sync records for each member
    FOR $member IN $room_members {
        CREATE matrix_sync_room_event SET
            user_id = $member.user_id,
            room_id = $this.room_id,
            event_id = $this.event_id,
            event_type = $this.event_type,
            sequence_number = $sequence,
            timeline_event = {},
            state_event = {
                event_id: $this.event_id,
                type: $this.event_type,
                sender: $this.sender,
                state_key: $this.state_key,
                content: $this.content,
                origin_server_ts: $this.origin_server_ts
            },
            created_at = time::now()
    }
);

-- Account data events for sync
DEFINE EVENT account_data_sync_event ON TABLE account_data
WHEN $event = "CREATE" OR $event = "UPDATE" THEN (
    LET $sequence = fn::next_sequence();

    CREATE matrix_sync_account_data SET
        user_id = $this.user_id,
        room_id = $this.room_id,
        data_type = $this.data_type,
        sequence_number = $sequence,
        content = $this.content,
        created_at = time::now()
);

-- Presence events for sync
DEFINE EVENT presence_sync_event ON TABLE user_presence_update
WHEN $event = "CREATE" OR $event = "UPDATE" THEN (
    LET $sequence = fn::next_sequence();

    -- Find users in shared rooms (presence subscribers)
    LET $shared_room_users = (
        SELECT DISTINCT rm2.user_id
        FROM room_membership rm1
        JOIN room_membership rm2 ON rm1.room_id = rm2.room_id
        WHERE rm1.user_id = $this.user_id
        AND rm2.user_id != $this.user_id
        AND rm1.membership = 'join'
        AND rm2.membership = 'join'
    );

    -- Create sync records for subscribers
    FOR $subscriber IN $shared_room_users {
        CREATE matrix_sync_presence SET
            user_id = $subscriber.user_id,
            presence_user_id = $this.user_id,
            sequence_number = $sequence,
            presence = $this.presence,
            last_active_ago = $this.last_active_ago,
            status_msg = $this.status_msg,
            currently_active = $this.currently_active,
            created_at = time::now()
    }
);

-- Device list events for sync
DEFINE EVENT device_sync_event ON TABLE device
WHEN $event = "CREATE" OR $event = "UPDATE" THEN (
    LET $sequence = fn::next_sequence();

    -- Find users in shared rooms (E2EE participants)
    LET $shared_room_users = (
        SELECT DISTINCT rm2.user_id
        FROM room_membership rm1
        JOIN room_membership rm2 ON rm1.room_id = rm2.room_id
        WHERE rm1.user_id = $this.user_id
        AND rm1.membership = 'join'
        AND rm2.membership = 'join'
    );

    -- Create sync records for E2EE participants
    FOR $participant IN $shared_room_users {
        CREATE matrix_sync_device_list SET
            user_id = $participant.user_id,
            changed_user_id = $this.user_id,
            sequence_number = $sequence,
            device_id = $this.device_id,
            created_at = time::now()
    }
);

-- To-device message events for sync
DEFINE EVENT to_device_sync_event ON TABLE to_device_messages
WHEN $event = "CREATE" THEN (
    LET $sequence = fn::next_sequence();

    CREATE matrix_sync_to_device SET
        user_id = $this.recipient_id,
        device_id = $this.device_id,
        sequence_number = $sequence,
        sender = $this.sender_id,
        message_type = $this.message_type,
        content = $this.content,
        created_at = time::now()
);

-- Typing notification events for ephemeral sync
DEFINE EVENT typing_sync_event ON TABLE typing_notification
WHEN $event = "CREATE" THEN (
    LET $sequence = fn::next_sequence();

    -- Find room members to notify (exclude typing user)
    LET $room_members = (
        SELECT user_id FROM room_membership
        WHERE room_id = $this.room_id
        AND membership = 'join'
        AND user_id != $this.user_id
    );

    -- Create ephemeral sync records
    FOR $member IN $room_members {
        CREATE matrix_sync_room_event SET
            user_id = $member.user_id,
            room_id = $this.room_id,
            event_id = NONE,
            event_type = 'm.typing',
            sequence_number = $sequence,
            timeline_event = {
                type: 'm.typing',
                content: {
                    user_id: $this.user_id,
                    typing: $this.typing
                }
            },
            state_event = {},
            created_at = time::now()
    }
);

-- Read receipt events for ephemeral sync
DEFINE EVENT receipt_sync_event ON TABLE receipts
WHEN $event = "CREATE" OR $event = "UPDATE" THEN (
    LET $sequence = fn::next_sequence();

    -- Find room members to notify
    LET $room_members = (
        SELECT user_id FROM room_membership
        WHERE room_id = $this.room_id
        AND membership = 'join'
    );

    -- Create ephemeral sync records
    FOR $member IN $room_members {
        CREATE matrix_sync_room_event SET
            user_id = $member.user_id,
            room_id = $this.room_id,
            event_id = $this.event_id,
            event_type = 'm.receipt',
            sequence_number = $sequence,
            timeline_event = {
                type: 'm.receipt',
                content: {
                    event_id: $this.event_id,
                    receipt_type: $this.receipt_type,
                    user_id: $this.user_id,
                    ts: $this.timestamp,
                    thread_id: $this.thread_id
                }
            },
            state_event = {},
            created_at = time::now()
    }
);


-- =====================================================
-- Migration: 096
-- Table: matrix_global_sequence
-- Purpose: Global sequence counter for sync event ordering
-- =====================================================

-- Global sequence counter for ordering all sync events
DEFINE TABLE matrix_global_sequence SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update WHERE $auth.admin = true
        FOR delete WHERE false;

DEFINE FIELD id ON TABLE matrix_global_sequence TYPE string DEFAULT 'global' ASSERT $value = 'global';
DEFINE FIELD sequence_number ON TABLE matrix_global_sequence TYPE int DEFAULT 0 ASSERT $value >= 0;
DEFINE FIELD updated_at ON TABLE matrix_global_sequence TYPE datetime DEFAULT time::now();

DEFINE INDEX matrix_global_sequence_id_idx ON TABLE matrix_global_sequence COLUMNS id UNIQUE;

-- Initialize the sequence counter
CREATE matrix_global_sequence:global SET
    id = 'global',
    sequence_number = 0,
    updated_at = time::now();


