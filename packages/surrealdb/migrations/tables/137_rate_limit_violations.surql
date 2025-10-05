-- =====================================================
-- Migration: 137
-- Table: rate_limit_violations
-- Entity: RateLimitViolation (captcha.rs line 34)
-- Repositories: captcha.rs
-- =====================================================

-- Rate limit violation tracking for security monitoring
DEFINE TABLE rate_limit_violations SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE true
        FOR delete WHERE $auth.admin = true;

DEFINE FIELD id ON TABLE rate_limit_violations TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD ip_address ON TABLE rate_limit_violations TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD operation ON TABLE rate_limit_violations TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD created_at ON TABLE rate_limit_violations TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (captcha.rs line 145)
DEFINE INDEX rate_limit_violations_id_idx ON TABLE rate_limit_violations COLUMNS id UNIQUE;
DEFINE INDEX rate_limit_violations_ip_operation_created_idx ON TABLE rate_limit_violations COLUMNS ip_address, operation, created_at;
