-- Enhanced device table with comprehensive metadata
-- Add new columns to existing device table
ALTER TABLE device ADD COLUMN trust_level STRING DEFAULT 'unverified';
ALTER TABLE device ADD COLUMN is_deleted BOOL DEFAULT false;
ALTER TABLE device ADD COLUMN stream_id INT DEFAULT 0;

-- Add vodozemac-specific fields for enhanced E2EE support
ALTER TABLE device_keys ADD COLUMN signature_valid BOOL DEFAULT NULL;
ALTER TABLE device_keys ADD COLUMN validation_timestamp DATETIME DEFAULT NULL;
ALTER TABLE device_keys ADD COLUMN vodozemac_account_data OPTION<OBJECT>;

-- Enhanced one-time keys with algorithm tracking
ALTER TABLE one_time_keys ADD COLUMN algorithm_type STRING DEFAULT 'signed_curve25519';
ALTER TABLE one_time_keys ADD COLUMN vodozemac_validated BOOL DEFAULT false;

-- Add indexes for the new columns
DEFINE INDEX user_device_idx ON TABLE device COLUMNS user_id, device_id;
DEFINE INDEX user_stream_idx ON TABLE device COLUMNS user_id, stream_id;
DEFINE INDEX activity_idx ON TABLE device COLUMNS user_id, last_seen_ts;

-- Device list update tracking
CREATE TABLE device_list_update (
    id RECORD(device_list_update),
    user_id STRING,
    device_id STRING,
    stream_id INT,
    prev_id ARRAY<INT>,
    deleted BOOL,
    update_data OBJECT,
    created_ts DATETIME DEFAULT time::now(),
    INDEX user_stream_idx ON (user_id, stream_id),
    INDEX dependency_idx ON (user_id, prev_id)
);

-- Cross-signing keys
CREATE TABLE cross_signing_key (
    id RECORD(cross_signing_key),
    user_id STRING,
    key_type STRING, -- 'master', 'self_signing', 'user_signing'
    key_data OBJECT,
    signatures OPTION<OBJECT>,
    created_ts DATETIME DEFAULT time::now(),
    INDEX user_type_idx ON (user_id, key_type)
);

-- Device metrics for monitoring
CREATE TABLE device_metrics (
    id RECORD(device_metrics),
    metric_name STRING,
    metric_value FLOAT,
    user_id OPTION<STRING>,
    device_id OPTION<STRING>,
    timestamp DATETIME DEFAULT time::now(),
    metadata OPTION<OBJECT>
);

-- Device verification records
CREATE TABLE device_verification (
    id RECORD(device_verification),
    user_id STRING,
    device_id STRING,
    verifier_user_id STRING,
    verification_method STRING, -- 'cross_signing', 'manual', 'tofu'
    trust_level STRING, -- 'unverified', 'cross_signed', 'verified', 'blacklisted'
    verified_at DATETIME DEFAULT time::now(),
    expires_at OPTION<DATETIME>,
    INDEX user_device_verification_idx ON (user_id, device_id),
    INDEX verifier_idx ON (verifier_user_id)
);

-- Cross-signing verification results
CREATE TABLE cross_signing_verification (
    id RECORD(cross_signing_verification),
    user_id STRING,
    master_key_id STRING,
    self_signing_key_id STRING,
    device_id STRING,
    verification_result STRING, -- 'valid', 'invalid', 'pending'
    verification_timestamp DATETIME DEFAULT time::now(),
    error_details OPTION<STRING>,
    INDEX user_verification_idx ON (user_id, verification_result),
    INDEX timestamp_idx ON (verification_timestamp)
);

-- Room key backups with vodozemac encryption
CREATE TABLE room_key_backups_v2 (
    id RECORD(room_key_backups_v2),
    user_id STRING,
    version STRING,
    room_id STRING,
    session_id STRING,
    encrypted_key_data OBJECT, -- vodozemac encrypted data
    backup_algorithm STRING DEFAULT 'm.megolm_backup.v1.curve25519-aes-sha2',
    created_ts DATETIME DEFAULT time::now(),
    INDEX user_version_room_idx ON (user_id, version, room_id),
    INDEX session_lookup_idx ON (user_id, version, room_id, session_id)
);

-- Backup versions with vodozemac auth data
CREATE TABLE room_key_backup_versions (
    id RECORD(room_key_backup_versions),
    user_id STRING,
    version STRING,
    algorithm STRING,
    auth_data OBJECT, -- vodozemac public key + signatures
    vodozemac_validated BOOL DEFAULT false,
    created_at STRING,
    active BOOL DEFAULT true,
    INDEX user_active_idx ON (user_id, active)
);

-- Room key backups
CREATE TABLE room_key_backups (
    id RECORD(room_key_backups),
    user_id STRING,
    backup_version STRING,
    room_id STRING,
    session_id STRING,
    encrypted_data OBJECT,
    created_at DATETIME DEFAULT time::now(),
    INDEX user_backup_idx ON (user_id, backup_version),
    INDEX room_session_idx ON (room_id, session_id)
);

-- Olm sessions managed by vodozemac
CREATE TABLE olm_sessions (
    id RECORD(olm_sessions),
    session_id STRING,
    user_id STRING,
    device_id STRING,
    their_curve25519_key STRING,
    vodozemac_session_data OBJECT, -- Serialized vodozemac Session
    created_ts DATETIME DEFAULT time::now(),
    last_used_ts DATETIME DEFAULT time::now(),
    INDEX session_lookup_idx ON (user_id, device_id, their_curve25519_key),
    INDEX session_id_idx ON (session_id)
);

-- Megolm group sessions
CREATE TABLE megolm_sessions (
    id RECORD(megolm_sessions),
    session_id STRING,
    room_id STRING,
    user_id STRING,
    vodozemac_group_session_data OBJECT, -- Serialized vodozemac GroupSession
    created_ts DATETIME DEFAULT time::now(),
    ratchet_count INT DEFAULT 0,
    INDEX room_session_idx ON (room_id, session_id),
    INDEX user_room_idx ON (user_id, room_id)
);

-- Federation key query cache
CREATE TABLE federation_key_cache (
    id RECORD(federation_key_cache),
    user_id STRING,
    server_name STRING,
    device_keys OBJECT,
    cross_signing_keys OBJECT,
    cached_ts DATETIME DEFAULT time::now(),
    expires_ts DATETIME,
    INDEX user_cache_idx ON (user_id),
    INDEX server_cache_idx ON (server_name),
    INDEX expiry_idx ON (expires_ts)
);
