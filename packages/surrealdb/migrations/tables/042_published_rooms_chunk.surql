-- =====================================================
-- Migration: 042
-- Table: published_rooms_chunk
-- Entity: packages/entity/src/types/published_rooms_chunk.rs
-- Repositories: [To be filled during repository analysis]
-- =====================================================

-- PublishedRoomsChunk table - matches PublishedRoomsChunk entity exactly
DEFINE TABLE published_rooms_chunk SCHEMAFULL
    PERMISSIONS
        FOR select WHERE true  -- Public room directory is readable by all
        FOR create, update, delete WHERE $auth.admin = true;

DEFINE FIELD room_id ON TABLE published_rooms_chunk TYPE string ASSERT string::is::not::empty($value);
DEFINE FIELD name ON TABLE published_rooms_chunk TYPE option<string>;
DEFINE FIELD topic ON TABLE published_rooms_chunk TYPE option<string>;
DEFINE FIELD canonical_alias ON TABLE published_rooms_chunk TYPE option<string>;
DEFINE FIELD num_joined_members ON TABLE published_rooms_chunk TYPE int DEFAULT 0;
DEFINE FIELD world_readable ON TABLE published_rooms_chunk TYPE bool DEFAULT false;
DEFINE FIELD guest_can_join ON TABLE published_rooms_chunk TYPE bool DEFAULT false;
DEFINE FIELD avatar_url ON TABLE published_rooms_chunk TYPE option<string>;
DEFINE FIELD room_type ON TABLE published_rooms_chunk TYPE option<string>;
