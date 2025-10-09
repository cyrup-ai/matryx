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

-- Ensure uses_remaining never exceeds uses_allowed
DEFINE FIELD uses_remaining ON TABLE registration_token TYPE option<int>
    ASSERT 
        $value IS NONE 
        OR $parent.uses_allowed IS NONE 
        OR $value <= $parent.uses_allowed;

-- Clarify state machine: token cannot be both pending and completed
-- Note: pending = registration in progress, completed = registration finished
-- A token can be neither (unused), pending (in use), or completed (consumed)

DEFINE FIELD expires_at ON TABLE registration_token TYPE option<datetime>;

DEFINE FIELD created_by ON TABLE registration_token TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD created_at ON TABLE registration_token TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns
DEFINE INDEX registration_token_token_idx ON TABLE registration_token COLUMNS token UNIQUE;
DEFINE INDEX registration_token_expires_idx ON TABLE registration_token COLUMNS expires_at;
