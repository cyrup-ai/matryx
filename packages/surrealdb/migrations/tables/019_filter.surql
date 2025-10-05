-- =====================================================
-- Migration: 019
-- Table: filter
-- Entity: packages/entity/src/types/filter.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Filter table - Matrix filtering criteria (matches Filter entity exactly)
DEFINE TABLE filter SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.user_id != NONE;  -- Filters are user-specific resources

DEFINE FIELD limit ON TABLE filter TYPE option<int>;
DEFINE FIELD types ON TABLE filter TYPE option<array<string>>;
DEFINE FIELD not_types ON TABLE filter TYPE option<array<string>>;
DEFINE FIELD senders ON TABLE filter TYPE option<array<string>>;
DEFINE FIELD not_senders ON TABLE filter TYPE option<array<string>>;
