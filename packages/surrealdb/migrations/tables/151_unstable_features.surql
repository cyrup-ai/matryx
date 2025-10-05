-- =====================================================
-- Migration: 151
-- Table: unstable_features
-- Entity: Unstable feature flags
-- Repositories: capabilities.rs
-- =====================================================

-- Unstable Matrix feature flags for experimental features
DEFINE TABLE unstable_features SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD feature_name ON TABLE unstable_features TYPE string 
    ASSERT string::is::not::empty($value);

DEFINE FIELD enabled ON TABLE unstable_features TYPE bool DEFAULT false;

DEFINE FIELD description ON TABLE unstable_features TYPE option<string>;

DEFINE FIELD updated_at ON TABLE unstable_features TYPE datetime DEFAULT time::now();

-- Indexes based on repository query patterns (capabilities.rs line 229)
DEFINE INDEX unstable_features_feature_idx ON TABLE unstable_features COLUMNS feature_name UNIQUE;
DEFINE INDEX unstable_features_enabled_idx ON TABLE unstable_features COLUMNS enabled;
