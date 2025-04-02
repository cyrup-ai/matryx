-- Matrix StateStore Schema for SurrealDB 2.2+
-- This migration creates all the necessary tables for the Matrix SDK StateStore

-- Room State table
DEFINE TABLE room_state SCHEMAFULL;
DEFINE FIELD id ON room_state TYPE option<string>;
DEFINE FIELD room_id ON room_state TYPE string;
DEFINE FIELD event_type ON room_state TYPE string;
DEFINE FIELD state_key ON room_state TYPE string;
DEFINE FIELD event ON room_state TYPE object;
DEFINE FIELD updated_at ON room_state TYPE datetime;

-- Create indices for efficient lookups
DEFINE INDEX room_state_idx_room ON room_state FIELDS room_id;
DEFINE INDEX room_state_idx_type ON room_state FIELDS event_type;
DEFINE INDEX room_state_idx_room_type ON room_state FIELDS room_id, event_type;
DEFINE INDEX room_state_idx_room_type_key ON room_state FIELDS room_id, event_type, state_key UNIQUE;

-- Account Data table
DEFINE TABLE account_data SCHEMAFULL;
DEFINE FIELD id ON account_data TYPE option<string>;
DEFINE FIELD event_type ON account_data TYPE string;
DEFINE FIELD room_id ON account_data TYPE option<string>;
DEFINE FIELD event ON account_data TYPE object;
DEFINE FIELD updated_at ON account_data TYPE datetime;

-- Create indices for account data
DEFINE INDEX account_data_idx_type ON account_data FIELDS event_type;
DEFINE INDEX account_data_idx_global ON account_data FIELDS event_type WHERE room_id IS NONE;
DEFINE INDEX account_data_idx_room_type ON account_data FIELDS event_type, room_id WHERE room_id IS NOT NONE UNIQUE;

-- Presence table
DEFINE TABLE presence SCHEMAFULL;
DEFINE FIELD id ON presence TYPE option<string>;
DEFINE FIELD user_id ON presence TYPE string;
DEFINE FIELD event ON presence TYPE object;
DEFINE FIELD updated_at ON presence TYPE datetime;

-- Create index for presence
DEFINE INDEX presence_idx_user ON presence FIELDS user_id UNIQUE;

-- Send Queue table
DEFINE TABLE send_queue SCHEMAFULL;
DEFINE FIELD id ON send_queue TYPE option<string>;
DEFINE FIELD queue_id ON send_queue TYPE string;
DEFINE FIELD request ON send_queue TYPE object;
DEFINE FIELD created_at ON send_queue TYPE datetime;

-- Create index for send queue
DEFINE INDEX send_queue_idx_id ON send_queue FIELDS queue_id UNIQUE;

-- Request Dependency table
DEFINE TABLE request_dependency SCHEMAFULL;
DEFINE FIELD id ON request_dependency TYPE option<string>;
DEFINE FIELD queue_id ON request_dependency TYPE string;
DEFINE FIELD dependent_id ON request_dependency TYPE string;
DEFINE FIELD created_at ON request_dependency TYPE datetime;

-- Create indices for request dependencies
DEFINE INDEX request_dependency_idx_queue ON request_dependency FIELDS queue_id;
DEFINE INDEX request_dependency_idx_unique ON request_dependency FIELDS queue_id, dependent_id UNIQUE;

-- Media Upload table
DEFINE TABLE media_upload SCHEMAFULL;
DEFINE FIELD id ON media_upload TYPE option<string>;
DEFINE FIELD request_id ON media_upload TYPE string;
DEFINE FIELD status ON media_upload TYPE string;
DEFINE FIELD started_at ON media_upload TYPE datetime;
DEFINE FIELD completed_at ON media_upload TYPE option<datetime>;

-- Create index for media uploads
DEFINE INDEX media_upload_idx_request ON media_upload FIELDS request_id UNIQUE;
DEFINE INDEX media_upload_idx_status ON media_upload FIELDS status;

-- API Cache table (already defined in initial schema, but adding here for completeness)
-- Used for storing sync tokens, filter IDs, and custom values
-- If api_cache table already exists, uncomment these:
-- DEFINE TABLE api_cache SCHEMAFULL;
-- DEFINE FIELD id ON api_cache TYPE option<string>;
-- DEFINE FIELD endpoint ON api_cache TYPE string;
-- DEFINE FIELD parameters ON api_cache TYPE object;
-- DEFINE FIELD response_data ON api_cache TYPE object;
-- DEFINE FIELD cached_at ON api_cache TYPE datetime;
-- DEFINE FIELD expires_at ON api_cache TYPE option<datetime>;
-- DEFINE FIELD etag ON api_cache TYPE option<string>;

-- Create indices for api_cache (uncomment if needed)
-- DEFINE INDEX api_cache_idx_endpoint ON api_cache FIELDS endpoint;
-- DEFINE INDEX api_cache_idx_matrix_cache ON api_cache FIELDS endpoint, parameters.key WHERE endpoint = 'matrix_cache' UNIQUE;