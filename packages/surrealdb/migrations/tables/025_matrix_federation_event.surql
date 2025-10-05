-- =====================================================
-- Migration: 025
-- Table: matrix_federation_event
-- Entity: packages/entity/src/types/matrix_federation_event.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- Matrix federation event queue for server-to-server communication
DEFINE TABLE matrix_federation_event SCHEMAFULL
    PERMISSIONS
        FOR select, create, update, delete WHERE $auth.admin = true;

DEFINE FIELD destination_server ON TABLE matrix_federation_event TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD event_id ON TABLE matrix_federation_event TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD room_id ON TABLE matrix_federation_event TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD sequence_number ON TABLE matrix_federation_event TYPE int ASSERT $value > 0;
DEFINE FIELD pdu_content ON TABLE matrix_federation_event TYPE object DEFAULT {};
DEFINE FIELD retry_count ON TABLE matrix_federation_event TYPE int DEFAULT 0;
DEFINE FIELD max_retry_count ON TABLE matrix_federation_event TYPE int DEFAULT 5;
DEFINE FIELD next_retry_at ON TABLE matrix_federation_event TYPE datetime DEFAULT time::now();
DEFINE FIELD last_error ON TABLE matrix_federation_event TYPE option<string>;
DEFINE FIELD status ON TABLE matrix_federation_event TYPE string DEFAULT 'pending' ASSERT $value IN ['pending', 'processing', 'sent', 'failed'];
DEFINE FIELD created_at ON TABLE matrix_federation_event TYPE datetime DEFAULT time::now();
