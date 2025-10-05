-- =====================================================
-- Migration: 002
-- Table: authentication_content
-- Entity: packages/entity/src/types/authentication_content.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- AuthenticationContent table - matches AuthenticationContent enum variants
DEFINE TABLE authentication_content SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD auth_type ON TABLE authentication_content TYPE string ASSERT $value IN ['m.login.password', 'm.login.recaptcha', 'm.login.token', 'm.login.dummy'];
DEFINE FIELD session ON TABLE authentication_content TYPE option<string>;
-- Password auth fields
DEFINE FIELD identifier ON TABLE authentication_content TYPE option<object>;
DEFINE FIELD password ON TABLE authentication_content TYPE option<string>;
-- Recaptcha auth fields  
DEFINE FIELD response ON TABLE authentication_content TYPE option<string>;
-- Token auth fields
DEFINE FIELD token ON TABLE authentication_content TYPE option<string>;
