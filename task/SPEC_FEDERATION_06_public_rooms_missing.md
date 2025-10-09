# SPEC_FEDERATION_06: Implement Public Rooms Endpoint

## Status
MISSING - Endpoint does not exist

## Description
The public rooms directory endpoint for federation is missing. This allows servers to query other servers for their public room lists.

## Spec Requirements (spec/server/13-public-rooms.md)

### Endpoint
`GET /_matrix/federation/v1/publicRooms`

### What's Required
1. Return list of public rooms on this server
2. Support pagination with since/limit
3. Filter by search terms if provided
4. Include room metadata (name, topic, aliases, member count, etc.)

### Query Parameters
- `limit`: Maximum number of rooms to return
- `since`: Pagination token from previous request
- `include_all_networks`: Include all third-party networks (default false)
- `third_party_instance_id`: Filter by specific third-party instance

### Response Format (200)
```json
{
  "chunk": [
    {
      "room_id": "!room:example.org",
      "num_joined_members": 42,
      "world_readable": true,
      "guest_can_join": true,
      "avatar_url": "mxc://example.org/avatar",
      "name": "Example Room",
      "topic": "A room about examples",
      "canonical_alias": "#example:example.org",
      "aliases": ["#example:example.org"]
    }
  ],
  "next_batch": "next_token",
  "prev_batch": "prev_token",
  "total_room_count_estimate": 115
}
```

## What Needs to be Done

1. **Create endpoint file**
   - Path: `/packages/server/src/_matrix/federation/v1/public_rooms.rs`
   - Handler: `pub async fn get(...)`

2. **Parse query parameters**
   - limit (optional, default 50)
   - since (pagination token)
   - include_all_networks (boolean)
   - third_party_instance_id

3. **Query public rooms**
   - Get rooms with visibility = "public"
   - Apply pagination
   - Apply search filter if provided
   - Sort by member count or activity

4. **Build room metadata**
   - Get room name from state
   - Get topic from state
   - Get canonical alias
   - Count joined members
   - Check world_readable
   - Check guest_can_join
   - Get avatar_url

5. **Generate pagination tokens**
   - Create next_batch token
   - Create prev_batch token
   - Include total estimate

6. **Return response**
   - Format chunk array
   - Include pagination tokens
   - Include room count estimate

7. **Register route**
   - Add to mod.rs
   - Wire into router

## Files to Create
- `/packages/server/src/_matrix/federation/v1/public_rooms.rs`
- Update `/packages/server/src/_matrix/federation/v1/mod.rs`

## Files to Reference
- Client API: `/packages/server/src/_matrix/client/v3/public_rooms.rs` (if exists)
- Room queries: Repository pattern for room metadata

## Verification
- [ ] Endpoint exists and responds
- [ ] Returns only public rooms
- [ ] Pagination works correctly
- [ ] Room metadata complete
- [ ] Member counts accurate
- [ ] Search filtering works
- [ ] Tokens generated properly
- [ ] Estimate included

## Priority
MEDIUM - Useful for room discovery across federation
