# Matrix Server-Server API - Public Rooms and Space Hierarchy

This specification covers the published room directory federation endpoints and space hierarchy functionality, allowing homeservers to query published rooms and space information from other servers.

## Published Room Directory

To complement the [room directory in the Client-Server API](https://spec.matrix.org/unstable/client-server-api/#published-room-directory), homeservers need a way to query the published rooms of another server. This can be done by making a request to the `/publicRooms` endpoint for the server the room directory should be retrieved for.

## GET /\_matrix/federation/v1/publicRooms

---

Lists the server's published room directory.

This API returns paginated responses. The rooms are ordered by the number of joined members, with the largest rooms first.

This SHOULD not return rooms that are listed on another homeserver's directory, just those listed on the receiving homeserver's directory.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `include_all_networks` | `boolean` | Whether or not to include all networks/protocols defined by application services on the homeserver. Defaults to false. |
| `limit` | `integer` | The maximum number of rooms to return. Defaults to 0 (no limit). |
| `since` | `string` | A pagination token from a previous call to this endpoint to fetch more rooms. |
| `third_party_instance_id` | `string` | The specific third-party network/protocol to request from the homeserver. Can only be used if `include_all_networks` is false.  This is the `instance_id` of a `Protocol Instance` returned by [`GET /_matrix/client/v3/thirdparty/protocols`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3thirdpartyprotocols). |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | A list of the published rooms on the server. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `chunk` | `[[PublishedRoomsChunk](https://spec.matrix.org/unstable/server-server-api/#get_matrixfederationv1publicrooms_response-200_publishedroomschunk)]` | **Required:** A paginated chunk of published rooms. |
| `next_batch` | `string` | A pagination token for the response. The absence of this token means there are no more results to fetch and the client should stop paginating. |
| `prev_batch` | `string` | A pagination token that allows fetching previous results. The absence of this token means there are no results before this batch, i.e. this is the first batch. |
| `total_room_count_estimate` | `integer` | An estimate on the total number of published rooms, if the server has an estimate. |

#### PublishedRoomsChunk

| Name | Type | Description |
| --- | --- | --- |
| `avatar_url` | `[URI](https://datatracker.ietf.org/doc/html/rfc3986)` | The URL for the room's avatar, if one is set. |
| `canonical_alias` | `[Room Alias](https://spec.matrix.org/unstable/appendices#room-aliases)` | The canonical alias of the room, if any. |
| `guest_can_join` | `boolean` | **Required:** Whether guest users may join the room and participate in it. If they can, they will be subject to ordinary power level rules like any other user. |
| `join_rule` | `string` | The room's join rule. When not present, the room is assumed to be `public`. |
| `name` | `string` | The name of the room, if any. |
| `num_joined_members` | `integer` | **Required:** The number of members joined to the room. |
| `room_id` | `[Room ID](https://spec.matrix.org/unstable/appendices#room-ids)` | **Required:** The ID of the room. |
| `room_type` | `string` | The `type` of room (from [`m.room.create`](https://spec.matrix.org/unstable/client-server-api/#mroomcreate)), if any. |
| `topic` | `string` | The plain text topic of the room. Omitted if no `text/plain` mimetype exists in [`m.room.topic`](https://spec.matrix.org/unstable/client-server-api/#mroomtopic). |
| `world_readable` | `boolean` | **Required:** Whether the room may be viewed by guest users without joining. |

### Example Response

```json
{
  "chunk": [
    {
      "avatar_url": "mxc://bleecker.street/CHEDDARandBRIE",
      "guest_can_join": false,
      "join_rule": "public",
      "name": "CHEESE",
      "num_joined_members": 37,
      "room_id": "!ol19s:bleecker.street",
      "room_type": "m.space",
      "topic": "Tasty tasty cheese",
      "world_readable": true
    }
  ],
  "next_batch": "p190q",
  "prev_batch": "p1902",
  "total_room_count_estimate": 115
}
```

## POST /\_matrix/federation/v1/publicRooms

---

Lists the server's published room directory with an optional filter.

This API returns paginated responses. The rooms are ordered by the number of joined members, with the largest rooms first.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `filter` | `[Filter](https://spec.matrix.org/unstable/server-server-api/#post_matrixfederationv1publicrooms_request_filter)` | Filter to apply to the results. |
| `include_all_networks` | `boolean` | Whether or not to include all known networks/protocols from application services on the homeserver. Defaults to false. |
| `limit` | `integer` | Limit the number of results returned. |
| `since` | `string` | A pagination token from a previous request, allowing servers to get the next (or previous) batch of rooms. The direction of pagination is specified solely by which token is supplied, rather than via an explicit flag. |
| `third_party_instance_id` | `string` | The specific third-party network/protocol to request from the homeserver. Can only be used if `include_all_networks` is false.  This is the `instance_id` of a `Protocol Instance` returned by [`GET /_matrix/client/v3/thirdparty/protocols`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3thirdpartyprotocols). |

#### Filter

| Name | Type | Description |
| --- | --- | --- |
| `generic_search_term` | `string` | An optional string to search for in the room metadata, e.g. name, topic, canonical alias, etc. |
| `room_types` | `[string\|null]` | An optional list of [room types](https://spec.matrix.org/unstable/client-server-api/#types) to search for. To include rooms without a room type, specify `null` within this list. When not specified, all applicable rooms (regardless of type) are returned.  **Added in `v1.4`** |

### Request body example

```json
{
  "filter": {
    "generic_search_term": "foo",
    "room_types": [
      null,
      "m.space"
    ]
  },
  "include_all_networks": false,
  "limit": 10,
  "third_party_instance_id": "irc-freenode"
}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | A filtered list of the published rooms on the server. |

### 200 response

The response format is identical to the GET version:

| Name | Type | Description |
| --- | --- | --- |
| `chunk` | `[[PublishedRoomsChunk](https://spec.matrix.org/unstable/server-server-api/#post_matrixfederationv1publicrooms_response-200_publishedroomschunk)]` | **Required:** A paginated chunk of published rooms. |
| `next_batch` | `string` | A pagination token for the response. The absence of this token means there are no more results to fetch and the client should stop paginating. |
| `prev_batch` | `string` | A pagination token that allows fetching previous results. The absence of this token means there are no results before this batch, i.e. this is the first batch. |
| `total_room_count_estimate` | `integer` | An estimate on the total number of published rooms, if the server has an estimate. |

### Example Response

```json
{
  "chunk": [
    {
      "avatar_url": "mxc://bleecker.street/CHEDDARandBRIE",
      "guest_can_join": false,
      "join_rule": "public",
      "name": "CHEESE",
      "num_joined_members": 37,
      "room_id": "!ol19s:bleecker.street",
      "room_type": "m.space",
      "topic": "Tasty tasty cheese",
      "world_readable": true
    }
  ],
  "next_batch": "p190q",
  "prev_batch": "p1902",
  "total_room_count_estimate": 115
}
```

## Spaces

To complement the [Client-Server API's Spaces module](https://spec.matrix.org/unstable/client-server-api/#spaces), homeservers need a way to query information about spaces from other servers.

## GET /\_matrix/federation/v1/hierarchy/{roomId}

---

**Added in `v1.2`**

Federation version of the Client-Server [`GET /hierarchy`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv1roomsroomidhierarchy) endpoint. Unlike the Client-Server API version, this endpoint does not paginate. Instead, all the space-room's children the requesting server could feasibly peek/join are returned. The requesting server is responsible for filtering the results further down for the user's request.

Only [`m.space.child`](https://spec.matrix.org/unstable/client-server-api/#mspacechild) state events of the room are considered. Invalid child rooms and parent events are not covered by this endpoint.

Responses to this endpoint should be cached for a period of time.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room ID of the space to get a hierarchy for. |

### Query parameters

| Name | Type | Description |
| --- | --- | --- |
| `suggested_only` | `boolean` | Optional (default `false`) flag to indicate whether or not the server should only consider suggested rooms. Suggested rooms are annotated in their [`m.space.child`](https://spec.matrix.org/unstable/client-server-api/#mspacechild) event contents. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The space room and its children. |
| `404` | The room is not known to the server or the requesting server is unable to peek/join it (if it were to attempt this). |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `children` | `[[SpaceHierarchyChildRoomsChunk](https://spec.matrix.org/unstable/server-server-api/#get_matrixfederationv1hierarchyroomid_response-200_spacehierarchychildroomschunk)]` | **Required:** A summary of the space's children. Rooms which the requesting server cannot peek/join will be excluded. |
| `inaccessible_children` | `[string]` | **Required:** The list of room IDs the requesting server doesn't have a viable way to peek/join. Rooms which the responding server cannot provide details on will be outright excluded from the response instead.  Assuming both the requesting and responding server are well behaved, the requesting server should consider these room IDs as not accessible from anywhere. They should not be re-requested. |
| `room` | `[SpaceHierarchyParentRoom](https://spec.matrix.org/unstable/server-server-api/#get_matrixfederationv1hierarchyroomid_response-200_spacehierarchyparentroom)` | **Required:** A summary of the room requested. |

#### SpaceHierarchyChildRoomsChunk

| Name | Type | Description |
| --- | --- | --- |
| `allowed_room_ids` | `[[Room ID](https://spec.matrix.org/unstable/appendices#room-ids)]` | If the room is a [restricted room](https://spec.matrix.org/unstable/server-server-api/#restricted-rooms), these are the room IDs which are specified by the join rules. Empty or omitted otherwise. |
| `avatar_url` | `[URI](https://datatracker.ietf.org/doc/html/rfc3986)` | The URL for the room's avatar, if one is set. |
| `canonical_alias` | `[Room Alias](https://spec.matrix.org/unstable/appendices#room-aliases)` | The canonical alias of the room, if any. |
| `children_state` | `[[StrippedStateEvent](https://spec.matrix.org/unstable/server-server-api/#get_matrixfederationv1hierarchyroomid_response-200_strippedstateevent)]` | **Required:** The [`m.space.child`](https://spec.matrix.org/unstable/client-server-api/#mspacechild) events of the space-room, represented as [Stripped State Events](https://spec.matrix.org/unstable/client-server-api/#stripped-state) with an added `origin_server_ts` key.  If the room is not a space-room, this should be empty. |
| `encryption` | `string` | The encryption algorithm to be used to encrypt messages sent in the room.  One of: `[m.megolm.v1.aes-sha2]`.  **Added in `v1.15`** |
| `guest_can_join` | `boolean` | **Required:** Whether guest users may join the room and participate in it. If they can, they will be subject to ordinary power level rules like any other user. |
| `join_rule` | `string` | The room's join rule. When not present, the room is assumed to be `public`. |
| `name` | `string` | The name of the room, if any. |
| `num_joined_members` | `integer` | **Required:** The number of members joined to the room. |
| `room_id` | `[Room ID](https://spec.matrix.org/unstable/appendices#room-ids)` | **Required:** The ID of the room. |
| `room_type` | `string` | The `type` of room (from [`m.room.create`](https://spec.matrix.org/unstable/client-server-api/#mroomcreate)), if any.  **Added in `v1.4`** |
| `room_version` | `string` | The version of the room.  **Added in `v1.15`** |
| `topic` | `string` | The plain text topic of the room. Omitted if no `text/plain` mimetype exists in [`m.room.topic`](https://spec.matrix.org/unstable/client-server-api/#mroomtopic). |
| `world_readable` | `boolean` | **Required:** Whether the room may be viewed by guest users without joining. |

#### SpaceHierarchyParentRoom

| Name | Type | Description |
| --- | --- | --- |
| `allowed_room_ids` | `[[Room ID](https://spec.matrix.org/unstable/appendices#room-ids)]` | If the room is a [restricted room](https://spec.matrix.org/unstable/server-server-api/#restricted-rooms), these are the room IDs which are specified by the join rules. Empty or omitted otherwise. |
| `avatar_url` | `[URI](https://datatracker.ietf.org/doc/html/rfc3986)` | The URL for the room's avatar, if one is set. |
| `canonical_alias` | `[Room Alias](https://spec.matrix.org/unstable/appendices#room-aliases)` | The canonical alias of the room, if any. |
| `children_state` | `[[StrippedStateEvent](https://spec.matrix.org/unstable/server-server-api/#get_matrixfederationv1hierarchyroomid_response-200_strippedstateevent)]` | **Required:** The [`m.space.child`](https://spec.matrix.org/unstable/client-server-api/#mspacechild) events of the space-room, represented as [Stripped State Events](https://spec.matrix.org/unstable/client-server-api/#stripped-state) with an added `origin_server_ts` key.  If the room is not a space-room, this should be empty. |
| `encryption` | `string` | The encryption algorithm to be used to encrypt messages sent in the room.  One of: `[m.megolm.v1.aes-sha2]`.  **Added in `v1.15`** |
| `guest_can_join` | `boolean` | **Required:** Whether guest users may join the room and participate in it. If they can, they will be subject to ordinary power level rules like any other user. |
| `join_rule` | `string` | The room's join rule. When not present, the room is assumed to be `public`. |
| `name` | `string` | The name of the room, if any. |
| `num_joined_members` | `integer` | **Required:** The number of members joined to the room. |
| `room_id` | `[Room ID](https://spec.matrix.org/unstable/appendices#room-ids)` | **Required:** The ID of the room. |
| `room_type` | `string` | The `type` of room (from [`m.room.create`](https://spec.matrix.org/unstable/client-server-api/#mroomcreate)), if any.  **Added in `v1.4`** |
| `room_version` | `string` | The version of the room.  **Added in `v1.15`** |
| `topic` | `string` | The plain text topic of the room. Omitted if no `text/plain` mimetype exists in [`m.room.topic`](https://spec.matrix.org/unstable/client-server-api/#mroomtopic). |
| `world_readable` | `boolean` | **Required:** Whether the room may be viewed by guest users without joining. |

#### StrippedStateEvent

| Name | Type | Description |
| --- | --- | --- |
| `content` | `EventContent` | **Required:** The `content` for the event. |
| `origin_server_ts` | `integer` | **Required:** The `origin_server_ts` for the event. |
| `sender` | `string` | **Required:** The `sender` for the event. |
| `state_key` | `string` | **Required:** The `state_key` for the event. |
| `type` | `string` | **Required:** The `type` for the event. |

### Example Response

```json
{
  "children": [
    {
      "allowed_room_ids": [
        "!upstream:example.org"
      ],
      "avatar_url": "mxc://example.org/abcdef2",
      "canonical_alias": "#general:example.org",
      "children_state": [
        {
          "content": {
            "via": [
              "remote.example.org"
            ]
          },
          "origin_server_ts": 1629422222222,
          "sender": "@alice:example.org",
          "state_key": "!b:example.org",
          "type": "m.space.child"
        }
      ],
      "guest_can_join": false,
      "join_rule": "restricted",
      "name": "The ~~First~~ Second Space",
      "num_joined_members": 42,
      "room_id": "!second_room:example.org",
      "room_type": "m.space",
      "topic": "Hello world",
      "world_readable": true
    }
  ],
  "inaccessible_children": [
    "!secret:example.org"
  ],
  "room": {
    "allowed_room_ids": [],
    "avatar_url": "mxc://example.org/abcdef",
    "canonical_alias": "#general:example.org",
    "children_state": [
      {
        "content": {
          "via": [
            "remote.example.org"
          ]
        },
        "origin_server_ts": 1629413349153,
        "sender": "@alice:example.org",
        "state_key": "!a:example.org",
        "type": "m.space.child"
      }
    ],
    "guest_can_join": false,
    "join_rule": "public",
    "name": "The First Space",
    "num_joined_members": 42,
    "room_id": "!space:example.org",
    "room_type": "m.space",
    "topic": "No other spaces were created first, ever",
    "world_readable": true
  }
}
```

### 404 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

### Example 404 Response

```json
{
  "errcode": "M_NOT_FOUND",
  "error": "Room does not exist."
}
```

## Implementation Considerations

### Public Room Directory

1. **Pagination**: Both GET and POST endpoints support pagination using `since`, `next_batch`, and `prev_batch` tokens.

2. **Filtering**: The POST endpoint allows advanced filtering including:
   - Generic search terms for room metadata
   - Room type filtering (including null for rooms without types)
   - Third-party network filtering

3. **Ordering**: Rooms should be ordered by member count (largest first) to prioritize active rooms.

4. **Server Restrictions**: Servers should only return rooms published on their own directory, not rooms from other servers.

### Space Hierarchy

1. **Caching**: Responses should be cached for performance as space hierarchies don't change frequently.

2. **Access Control**: Only return children that the requesting server can feasibly peek or join.

3. **State Events**: Only `m.space.child` events are considered for building the hierarchy.

4. **Stripped State**: Child room state events are returned as stripped state events with added `origin_server_ts`.

5. **Recursive Handling**: The endpoint doesn't paginate - it returns all accessible children in one response.