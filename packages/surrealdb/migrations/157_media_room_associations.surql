-- Media-Room Association Table
-- Links media uploads to rooms for access control
DEFINE TABLE media_room_associations SCHEMAFULL;
DEFINE FIELD media_id ON media_room_associations TYPE string;
DEFINE FIELD server_name ON media_room_associations TYPE string;
DEFINE FIELD room_id ON media_room_associations TYPE string;
DEFINE FIELD event_id ON media_room_associations TYPE option<string>;
DEFINE FIELD uploaded_by ON media_room_associations TYPE string;
DEFINE FIELD uploaded_at ON media_room_associations TYPE datetime;
DEFINE FIELD is_profile_picture ON media_room_associations TYPE bool DEFAULT false;

-- Indexes for fast lookups
DEFINE INDEX media_lookup_idx ON media_room_associations FIELDS media_id, server_name;
DEFINE INDEX room_media_idx ON media_room_associations FIELDS room_id;
DEFINE INDEX user_media_idx ON media_room_associations FIELDS uploaded_by;
