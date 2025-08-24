-- Initialize schema
-- Create the rooms table for storing room info
CREATE TABLE room_state (
    id RECORD,
    room_id STRING,
    event_type STRING,
    state_key STRING, 
    event OBJECT,
    created_at DATETIME,
    updated_at DATETIME
);

-- Create indexes for room_state table
DEFINE INDEX room_state_room_id ON TABLE room_state COLUMNS room_id;
DEFINE INDEX room_state_event_type ON TABLE room_state COLUMNS event_type;
DEFINE INDEX room_state_unique ON TABLE room_state COLUMNS room_id, event_type, state_key UNIQUE;

-- Create the account_data table
CREATE TABLE account_data (
    id RECORD,
    event_type STRING,
    room_id STRING OPTION,
    event OBJECT,
    updated_at DATETIME
);

-- Create indexes for account_data table
DEFINE INDEX account_data_room_id ON TABLE account_data COLUMNS room_id;
DEFINE INDEX account_data_event_type ON TABLE account_data COLUMNS event_type;
DEFINE INDEX account_data_unique ON TABLE account_data COLUMNS event_type, room_id UNIQUE;

-- Create the presence table
CREATE TABLE presence (
    id RECORD,
    user_id STRING,
    event OBJECT,
    updated_at DATETIME
);

-- Create index for presence table
DEFINE INDEX presence_user_id ON TABLE presence COLUMNS user_id UNIQUE;

-- Create the api_cache table for general caching
CREATE TABLE api_cache (
    id RECORD,
    key STRING,
    value STRING,
    updated_at DATETIME
);

-- Create index for api_cache table
DEFINE INDEX api_cache_key ON TABLE api_cache COLUMNS key UNIQUE;

-- Create send queue tables
CREATE TABLE send_queue_request (
    id RECORD,
    room_id STRING,
    transaction_id STRING,
    created_at DATETIME,
    kind STRING,
    content OBJECT,
    priority NUMBER,
    error OBJECT OPTION,
    updated_at DATETIME
);

-- Create indexes for send_queue_request table
DEFINE INDEX send_queue_room_id ON TABLE send_queue_request COLUMNS room_id;
DEFINE INDEX send_queue_transaction_id ON TABLE send_queue_request COLUMNS transaction_id;
DEFINE INDEX send_queue_unique ON TABLE send_queue_request COLUMNS room_id, transaction_id UNIQUE;

-- Create request dependency table
CREATE TABLE request_dependency (
    id RECORD,
    room_id STRING,
    parent_txn_id STRING,
    child_txn_id STRING,
    created_at DATETIME,
    kind STRING,
    content OBJECT,
    sent_parent_key OBJECT OPTION,
    updated_at DATETIME
);

-- Create indexes for request_dependency table
DEFINE INDEX request_dependency_room_id ON TABLE request_dependency COLUMNS room_id;
DEFINE INDEX request_dependency_parent_id ON TABLE request_dependency COLUMNS parent_txn_id;
DEFINE INDEX request_dependency_child_id ON TABLE request_dependency COLUMNS child_txn_id;
DEFINE INDEX request_dependency_unique ON TABLE request_dependency COLUMNS room_id, parent_txn_id, child_txn_id UNIQUE;

-- Create media upload table
CREATE TABLE media_upload (
    id RECORD,
    request_id STRING,
    started_at DATETIME
);

-- Create index for media_upload table
DEFINE INDEX media_upload_request_id ON TABLE media_upload COLUMNS request_id UNIQUE;

-- Create receipts table
CREATE TABLE receipts (
    id RECORD,
    room_id STRING,
    receipt_type STRING, 
    thread STRING,
    event_id STRING,
    user_id STRING,
    receipt_data OBJECT,
    updated_at DATETIME
);

-- Create indexes for receipts table
DEFINE INDEX receipts_room_id ON TABLE receipts COLUMNS room_id;
DEFINE INDEX receipts_event_id ON TABLE receipts COLUMNS event_id; 
DEFINE INDEX receipts_user_id ON TABLE receipts COLUMNS user_id;
DEFINE INDEX receipts_unique_user ON TABLE receipts COLUMNS room_id, receipt_type, thread, user_id UNIQUE;