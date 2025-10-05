-- =====================================================
-- Migration: 124
-- Table: registration_token
-- Entity: RegistrationToken (repository struct)
-- Repositories: registration.rs
-- =====================================================

-- Registration token validation for controlled user registration
DEFINE TABLE registration_token SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD token ON TABLE registration_token TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD uses_allowed ON TABLE registration_token TYPE option<int>;

DEFINE FIELD uses_remaining ON TABLE registration_token TYPE option<int>;

DEFINE FIELD pending ON TABLE registration_token TYPE bool DEFAULT false;

DEFINE FIELD completed ON TABLE registration_token TYPE bool DEFAULT false;

DEFINE FIELD expires_at ON TABLE registration_token TYPE option<datetime>;

DEFINE FIELD created_by ON TABLE registration_token TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD created_at ON TABLE registration_token TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns
DEFINE INDEX registration_token_token_idx ON TABLE registration_token COLUMNS token UNIQUE;
DEFINE INDEX registration_token_expires_idx ON TABLE registration_token COLUMNS expires_at;
