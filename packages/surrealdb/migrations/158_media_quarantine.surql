-- Add quarantine support to media_info table
DEFINE FIELD quarantined ON media_info TYPE bool DEFAULT false;
DEFINE FIELD quarantined_by ON media_info TYPE option<string>;
DEFINE FIELD quarantine_reason ON media_info TYPE option<string>;
DEFINE FIELD quarantined_at ON media_info TYPE option<datetime>;
DEFINE INDEX quarantined_idx ON media_info FIELDS quarantined;
