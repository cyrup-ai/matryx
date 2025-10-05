-- =====================================================
-- Migration: 126
-- Table: captcha_challenges
-- Entity: CaptchaChallenge (repository struct)
-- Repositories: captcha.rs
-- =====================================================

-- CAPTCHA challenge tracking for anti-abuse and bot prevention
DEFINE TABLE captcha_challenges SCHEMAFULL
    PERMISSIONS
        FOR select WHERE $auth.admin = true
        FOR create WHERE true
        FOR update, delete WHERE $auth.admin = true;

DEFINE FIELD challenge_id ON TABLE captcha_challenges TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD challenge_type ON TABLE captcha_challenges TYPE string 
    ASSERT $value IN ['recaptcha', 'hcaptcha', 'custom', 'math'];

DEFINE FIELD site_key ON TABLE captcha_challenges TYPE option<string>;

DEFINE FIELD question ON TABLE captcha_challenges TYPE option<string>;

DEFINE FIELD answer ON TABLE captcha_challenges TYPE option<string>;

DEFINE FIELD created_at ON TABLE captcha_challenges TYPE datetime DEFAULT time::now();

DEFINE FIELD expires_at ON TABLE captcha_challenges TYPE datetime;

DEFINE FIELD used ON TABLE captcha_challenges TYPE bool DEFAULT false;

DEFINE FIELD verified ON TABLE captcha_challenges TYPE bool DEFAULT false;

DEFINE FIELD user_id ON TABLE captcha_challenges TYPE option<string>;

DEFINE FIELD client_ip ON TABLE captcha_challenges TYPE option<string>;

DEFINE FIELD user_agent ON TABLE captcha_challenges TYPE option<string>;

DEFINE FIELD session_id ON TABLE captcha_challenges TYPE option<string>;

-- Indexes based on repository query patterns
DEFINE INDEX captcha_challenges_challenge_id_idx ON TABLE captcha_challenges COLUMNS challenge_id UNIQUE;
DEFINE INDEX captcha_challenges_client_ip_created_idx ON TABLE captcha_challenges COLUMNS client_ip, created_at;
DEFINE INDEX captcha_challenges_created_idx ON TABLE captcha_challenges COLUMNS created_at;
DEFINE INDEX captcha_challenges_expires_idx ON TABLE captcha_challenges COLUMNS expires_at;
