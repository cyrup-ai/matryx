-- =====================================================
-- Migration: 125
-- Table: registration_attempt
-- Entity: RegistrationAttempt (repository struct)
-- Repositories: registration.rs
-- =====================================================

-- Registration attempt tracking for rate limiting and security
DEFINE TABLE registration_attempt SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true
        FOR create WHERE true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD ip_address ON TABLE registration_attempt TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD timestamp ON TABLE registration_attempt TYPE datetime DEFAULT time::now();

DEFINE FIELD success ON TABLE registration_attempt TYPE bool DEFAULT false;

DEFINE FIELD user_agent ON TABLE registration_attempt TYPE option<string>;

DEFINE FIELD error_reason ON TABLE registration_attempt TYPE option<string>;

-- Indexes based on repository query patterns
DEFINE INDEX registration_attempt_ip_timestamp_idx ON TABLE registration_attempt COLUMNS ip_address, timestamp;
DEFINE INDEX registration_attempt_timestamp_idx ON TABLE registration_attempt COLUMNS timestamp;
