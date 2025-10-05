-- =====================================================
-- Migration: 152
-- Table: device_trust
-- Entity: Device trust levels for E2E encryption
-- Repositories: cross_signing.rs
-- =====================================================

-- Device trust tracking for cross-signing
DEFINE TABLE device_trust SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD user_id ON TABLE device_trust TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD device_id ON TABLE device_trust TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD trust_level ON TABLE device_trust TYPE string 
    ASSERT $value IN ['verified', 'cross_signed', 'unverified', 'blocked'];

DEFINE FIELD verified_by ON TABLE device_trust TYPE option<string>;

DEFINE FIELD verified_at ON TABLE device_trust TYPE option<datetime>;

DEFINE FIELD updated_at ON TABLE device_trust TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (cross_signing.rs line 309)
DEFINE INDEX device_trust_user_device_idx ON TABLE device_trust COLUMNS user_id, device_id UNIQUE;
DEFINE INDEX device_trust_user_trust_idx ON TABLE device_trust COLUMNS user_id, trust_level;
