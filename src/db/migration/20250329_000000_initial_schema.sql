-- Initial Matrix schema setup
-- Room membership table
DEFINE TABLE room_membership SCHEMAFULL;
DEFINE FIELD user_id ON room_membership TYPE string;
DEFINE FIELD room_id ON room_membership TYPE string;
DEFINE FIELD display_name ON room_membership TYPE option<string>;
DEFINE FIELD membership_status ON room_membership TYPE string;
DEFINE FIELD joined_at ON room_membership TYPE datetime;
DEFINE FIELD updated_at ON room_membership TYPE datetime;

DEFINE INDEX room_membership_user_idx ON room_membership COLUMNS user_id;
DEFINE INDEX room_membership_room_idx ON room_membership COLUMNS room_id;

-- Message history table
DEFINE TABLE message SCHEMAFULL;
DEFINE FIELD room_id ON message TYPE string;
DEFINE FIELD sender_id ON message TYPE string;
DEFINE FIELD content ON message TYPE string;
DEFINE FIELD message_type ON message TYPE string;
DEFINE FIELD sent_at ON message TYPE datetime;
DEFINE FIELD edited_at ON message TYPE option<datetime>;
DEFINE FIELD reactions ON message TYPE array<object>;

DEFINE INDEX message_time_idx ON message COLUMNS room_id, sent_at;
DEFINE INDEX message_sender_idx ON message COLUMNS sender_id, sent_at;

-- User profile table
DEFINE TABLE user_profile SCHEMAFULL;
DEFINE FIELD user_id ON user_profile TYPE string;
DEFINE FIELD display_name ON user_profile TYPE option<string>;
DEFINE FIELD avatar_url ON user_profile TYPE option<string>;
DEFINE FIELD email ON user_profile TYPE option<string>;
DEFINE FIELD presence ON user_profile TYPE string;
DEFINE FIELD last_active ON user_profile TYPE datetime;
DEFINE FIELD devices ON user_profile TYPE array<object>;
DEFINE FIELD settings ON user_profile TYPE object;

DEFINE INDEX user_profile_id_idx ON user_profile COLUMNS user_id UNIQUE;

-- API cache table
DEFINE TABLE api_cache SCHEMAFULL;
DEFINE FIELD endpoint ON api_cache TYPE string;
DEFINE FIELD parameters ON api_cache TYPE object;
DEFINE FIELD response_data ON api_cache TYPE any;
DEFINE FIELD cached_at ON api_cache TYPE datetime;
DEFINE FIELD expires_at ON api_cache TYPE option<datetime>;
DEFINE FIELD etag ON api_cache TYPE option<string>;

DEFINE INDEX api_cache_endpoint_idx ON api_cache COLUMNS endpoint, parameters;
DEFINE INDEX api_cache_expiry_idx ON api_cache COLUMNS expires_at;

-- Encryption data table
DEFINE TABLE encryption_data SCHEMAFULL;
DEFINE FIELD user_id ON encryption_data TYPE string;
DEFINE FIELD device_id ON encryption_data TYPE string;
DEFINE FIELD keys ON encryption_data TYPE object;
DEFINE FIELD signatures ON encryption_data TYPE object;
DEFINE FIELD verification_status ON encryption_data TYPE string;
DEFINE FIELD updated_at ON encryption_data TYPE datetime;

DEFINE INDEX encryption_user_device_idx ON encryption_data COLUMNS user_id, device_id UNIQUE;

-- Searchable message table with vector support
DEFINE TABLE searchable_message SCHEMAFULL;
DEFINE FIELD message_id ON searchable_message TYPE string;
DEFINE FIELD room_id ON searchable_message TYPE string;
DEFINE FIELD sender_id ON searchable_message TYPE string;
DEFINE FIELD content ON searchable_message TYPE string;
DEFINE FIELD sent_at ON searchable_message TYPE datetime;
DEFINE FIELD embedding ON searchable_message TYPE array<float>;

DEFINE INDEX message_content_idx ON searchable_message FULLTEXT content;
DEFINE INDEX message_vector_idx ON searchable_message VECTOR embedding 384 cosine;