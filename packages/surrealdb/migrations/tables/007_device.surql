-- =====================================================
-- Migration: 007
-- Table: device
-- Entity: packages/entity/src/types/device.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Device table - Matrix spec compliant (spec/client/04_security_encryption.md:247-250)
DEFINE TABLE device SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Devices are accessible to authenticated users managing them
        FOR create, update, delete WHERE $auth.user_id != NONE;

DEFINE FIELD device_id ON TABLE device TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD user_id ON TABLE device TYPE string ASSERT string::is::not::empty($value) AND string::starts_with($value, '@') AND string::contains($value, ':');
DEFINE FIELD display_name ON TABLE device TYPE option<string>;
DEFINE FIELD last_seen_ip ON TABLE device TYPE option<string>;
DEFINE FIELD last_seen_ts ON TABLE device TYPE option<int>;

DEFINE INDEX device_id_idx ON TABLE device COLUMNS device_id UNIQUE;
DEFINE INDEX device_user_idx ON TABLE device COLUMNS user_id;
DEFINE INDEX device_user_device_idx ON TABLE device COLUMNS user_id, device_id UNIQUE;
