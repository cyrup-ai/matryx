# SPEC_FEDERATION_07: Implement Query Directory Endpoint

## Status
MISSING - Endpoint does not exist

## Description
The directory query endpoint is missing. This resolves room aliases to room IDs across federation.

## Spec Requirements (spec/server/25-querying-information.md)

### Endpoint
`GET /_matrix/federation/v1/query/directory`

### What's Required
1. Resolve room alias to room ID
2. Return list of resident servers
3. Used during room joins for server discovery

### Query Parameters
- `room_alias`: The room alias to look up (required)

### Response Format (200)
```json
{
  "room_id": "!roomid:example.org",
  "servers": [
    "example.org",
    "example.com",
    "another.example.org"
  ]
}
```

## What Needs to be Done

1. **Create endpoint file**
   - Path: `/packages/server/src/_matrix/federation/v1/query/directory.rs`
   - Handler: `pub async fn get(...)`

2. **Parse query parameters**
   - Extract room_alias
   - Validate alias format (#localpart:domain)

3. **Validate request**
   - Parse X-Matrix auth
   - Verify server signature
   - Check alias domain matches our server

4. **Resolve room alias**
   - Query room_aliases table
   - Get associated room_id
   - Return 404 if not found

5. **Get resident servers**
   - Query servers with users in room
   - Sort by activity/reliability
   - Return list of server names

6. **Return response**
   - Include room_id
   - Include servers array

7. **Register route**
   - Add to query/mod.rs
   - Wire into router

## Files to Create
- `/packages/server/src/_matrix/federation/v1/query/directory.rs`
- Update `/packages/server/src/_matrix/federation/v1/query/mod.rs`

## Files to Reference
- Existing query: `/packages/server/src/_matrix/federation/v1/query/profile.rs`
- Room aliases: Repository for alias resolution

## Error Responses

### 404 - Not Found
```json
{
  "errcode": "M_NOT_FOUND",
  "error": "Room alias not found"
}
```

## Verification
- [ ] Endpoint exists and responds
- [ ] room_alias parameter parsed
- [ ] Alias format validated
- [ ] Room lookup works
- [ ] Servers list populated
- [ ] 404 for unknown aliases
- [ ] Only resolves local aliases

## Priority
HIGH - Critical for room joins via alias
