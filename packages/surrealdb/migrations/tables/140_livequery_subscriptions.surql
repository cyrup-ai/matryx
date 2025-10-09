-- =====================================================
-- Migration: 140
-- Table: livequery_subscriptions
-- Entity: LiveQuery subscription tracking
-- Repositories: session.rs
-- =====================================================

-- LiveQuery subscription tracking for real-time Matrix /sync
DEFINE TABLE livequery_subscriptions SCHEMAFULL
    PERMISSIONS
        FOR select WHERE user_id = $auth.user_id OR $auth.admin = true OR $auth.monitoring = true
        FOR create WHERE $auth.user_id != NONE
        FOR delete WHERE user_id = $auth.user_id OR $auth.admin = true;

DEFINE FIELD subscription_id ON TABLE livequery_subscriptions TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD user_id ON TABLE livequery_subscriptions TYPE string 
    ASSERT string::is::not::empty($value) 
    AND string::starts_with($value, '@') 
    AND string::contains($value, ':');

DEFINE FIELD room_ids ON TABLE livequery_subscriptions TYPE array<string> DEFAULT [];

DEFINE FIELD created_at ON TABLE livequery_subscriptions TYPE datetime DEFAULT time::now();

DEFINE FIELD last_active ON TABLE livequery_subscriptions TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns
DEFINE INDEX livequery_subscriptions_subscription_id_idx ON TABLE livequery_subscriptions COLUMNS subscription_id UNIQUE;
DEFINE INDEX livequery_subscriptions_user_idx ON TABLE livequery_subscriptions COLUMNS user_id;
DEFINE INDEX livequery_subscriptions_last_active_idx ON TABLE livequery_subscriptions COLUMNS last_active;
DEFINE INDEX livequery_subscriptions_created_idx ON TABLE livequery_subscriptions COLUMNS created_at;
