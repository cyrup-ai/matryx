-- =====================================================
-- Migration: 120
-- Table: third_party_identifiers
-- Entity: Third-party identifier mapping
-- Repositories: third_party.rs
-- =====================================================

-- Third-party identifier mapping for email/phone verification
DEFINE TABLE third_party_identifiers SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true
        FOR create, update, delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD user_id ON TABLE third_party_identifiers TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD medium ON TABLE third_party_identifiers TYPE string 
    ASSERT $value IN ['email', 'msisdn'];

DEFINE FIELD address ON TABLE third_party_identifiers TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD validated_at ON TABLE third_party_identifiers TYPE option<datetime>;

DEFINE FIELD created_at ON TABLE third_party_identifiers TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns
DEFINE INDEX third_party_identifiers_user_idx ON TABLE third_party_identifiers COLUMNS user_id;
DEFINE INDEX third_party_identifiers_user_medium_address_idx ON TABLE third_party_identifiers COLUMNS user_id, medium, address UNIQUE;
DEFINE INDEX third_party_identifiers_medium_address_idx ON TABLE third_party_identifiers COLUMNS medium, address;
