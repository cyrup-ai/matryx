# Room Joining

## Overview

Room joining in Matrix federation involves a multi-step handshake process between the joining server and a resident server in the target room. This process ensures proper authorization and provides the joining server with the necessary room state information.

## Room Joining Flow

The room joining process follows this sequence:

```
+---------+          +---------------+            +-----------------+ +-----------------+
| Client  |          | Joining       |            | Directory       | | Resident        |
|         |          | Server        |            | Server          | | Server          |
+---------+          +---------------+            +-----------------+ +-----------------+
     |                       |                             |                   |
     | join request          |                             |                   |
     |---------------------->|                             |                   |
     |                       |                             |                   |
     |                       | directory request           |                   |
     |                       |---------------------------->|                   |
     |                       |                             |                   |
     |                       |          directory response |                   |
     |                       |<----------------------------|                   |
     |                       |                             |                   |
     |                       | make_join request           |                   |
     |                       |------------------------------------------------>|
     |                       |                             |                   |
     |                       |                             |make_join response |
     |                       |<------------------------------------------------|
     |                       |                             |                   |
     |                       | send_join request           |                   |
     |                       |------------------------------------------------>|
     |                       |                             |                   |
     |                       |                             |send_join response |
     |                       |<------------------------------------------------|
     |                       |                             |                   |
     |         join response |                             |                   |
     |<----------------------|                             |                   |
     |                       |                             |                   |
```

## Joining Process Steps

### 1. Directory Resolution

The first part of the handshake usually involves using the directory server to request the room ID and join candidates through the [`/query/directory`](https://spec.matrix.org/unstable/server-server-api/#get_matrixfederationv1querydirectory) API endpoint. In the case of a new user joining a room as a result of a received invite, the joining user's homeserver could optimise this step away by picking the origin server of that invite message as the join candidate. However, the joining server should be aware that the origin server of the invite might since have left the room, so should be prepared to fall back on the regular join flow if this optimisation fails.

### 2. Make Join Request

Once the joining server has the room ID and the join candidates, it then needs to obtain enough information about the room to fill in the required fields of the `m.room.member` event. It obtains this by selecting a resident from the candidate list, and using the `GET /make_join` endpoint. The resident server will then reply with enough information for the joining server to fill in the event.

### 3. Event Preparation

The joining server is expected to add or replace the `origin`, `origin_server_ts`, and `event_id` on the templated event received by the resident server. This event is then signed by the joining server.

### 4. Send Join Request

To complete the join handshake, the joining server submits this new event to the resident server it used for `GET /make_join`, using the `PUT /send_join` endpoint.

### 5. Event Acceptance

The resident homeserver then adds its signature to this event and accepts it into the room's event graph. The joining server receives the full set of state for the newly-joined room as well as the freshly signed membership event. The resident server must also send the event to other servers participating in the room.

## GET /_matrix/federation/v1/make_join/{roomId}/{userId}

Asks the receiving server to return information that the sending server will need to prepare a join event to get into the room.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

### Request Parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room ID that is about to be joined. |
| `userId` | `string` | **Required:** The user ID the join event will be for. |

### Query Parameters

| Name | Type | Description |
| --- | --- | --- |
| `ver` | `[string]` | The room versions the sending server has support for. Defaults to `[1]`. |

### Responses

| Status | Description |
| --- | --- |
| `200` | A template to be used for the rest of the [Joining Rooms](https://spec.matrix.org/unstable/server-server-api/#joining-rooms) handshake. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. **The response body here describes the common event fields in more detail and may be missing other required fields for a PDU.** |
| `400` | The request is invalid, the room the server is attempting to join has a version that is not listed in the `ver` parameters, or the server was unable to validate [restricted room conditions](https://spec.matrix.org/unstable/server-server-api/#restricted-rooms). The error should be passed through to clients so that they may give better feedback to users. New in `v1.2`, the following error conditions might happen: If the room is [restricted](https://spec.matrix.org/unstable/client-server-api/#restricted-rooms) and none of the conditions can be validated by the server then the `errcode` `M_UNABLE_TO_AUTHORISE_JOIN` must be used. This can happen if the server does not know about any of the rooms listed as conditions, for example. `M_UNABLE_TO_GRANT_JOIN` is returned to denote that a different server should be attempted for the join. This is typically because the resident server can see that the joining user satisfies one or more conditions, such as in the case of [restricted rooms](https://spec.matrix.org/unstable/client-server-api/#restricted-rooms), but the resident server would be unable to meet the auth rules governing `join_authorised_via_users_server` on the resulting `m.room.member` event. |
| `403` | The room that the joining server is attempting to join does not permit the user to join. |
| `404` | The room that the joining server is attempting to join is unknown to the receiving server. |

### 200 Response

| Name | Type | Description |
| --- | --- | --- |
| `event` | `Event Template` | An unsigned template event. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. |
| `room_version` | `string` | The version of the room where the server is trying to join. If not provided, the room version is assumed to be either "1" or "2". |

#### Event Template

| Name | Type | Description |
| --- | --- | --- |
| `content` | `Membership Event Content` | **Required:** The content of the event. |
| `origin` | `string` | **Required:** The name of the resident homeserver. |
| `origin_server_ts` | `integer` | **Required:** A timestamp added by the resident homeserver. |
| `sender` | `string` | **Required:** The user ID of the joining member. |
| `state_key` | `string` | **Required:** The user ID of the joining member. |
| `type` | `string` | **Required:** The value `m.room.member`. |

#### Membership Event Content

| Name | Type | Description |
| --- | --- | --- |
| `join_authorised_via_users_server` | `string` | Required if the room is [restricted](https://spec.matrix.org/unstable/client-server-api/#restricted-rooms) and is joining through one of the conditions available. If the user is responding to an invite, this is not required. An arbitrary user ID belonging to the resident server in the room being joined that is able to issue invites to other users. This is used in later validation of the auth rules for the `m.room.member` event. **Added in `v1.2`** |
| `membership` | `string` | **Required:** The value `join`. |

#### Example

```json
{
  "event": {
    "content": {
      "join_authorised_via_users_server": "@anyone:resident.example.org",
      "membership": "join"
    },
    "origin": "example.org",
    "origin_server_ts": 1549041175876,
    "room_id": "!somewhere:example.org",
    "sender": "@someone:example.org",
    "state_key": "@someone:example.org",
    "type": "m.room.member"
  },
  "room_version": "2"
}
```

## PUT /_matrix/federation/v1/send_join/{roomId}/{eventId}

Submits a signed join event to a resident server for it to accept it into the room's graph. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

### Request Parameters

| Name | Type | Description |
| --- | --- |
| `eventId` | `string` | **Required:** The event ID for the join event. |
| `roomId` | `string` | **Required:** The room ID that is about to be joined. |

### Request Body

| Name | Type | Description |
| --- | --- | --- |
| `content` | `Membership Event Content` | **Required:** The content of the event. |
| `origin` | `string` | **Required:** The name of the joining homeserver. |
| `origin_server_ts` | `integer` | **Required:** A timestamp added by the joining homeserver. |
| `sender` | `string` | **Required:** The user ID of the joining member. |
| `state_key` | `string` | **Required:** The user ID of the joining member. |
| `type` | `string` | **Required:** The value `m.room.member`. |

#### Membership Event Content

| Name | Type | Description |
| --- | --- | --- |
| `join_authorised_via_users_server` | `string` | Required if the room is [restricted](https://spec.matrix.org/unstable/client-server-api/#restricted-rooms) and is joining through one of the conditions available. If the user is responding to an invite, this is not required. An arbitrary user ID belonging to the resident server in the room being joined that is able to issue invites to other users. This is used in later validation of the auth rules for the `m.room.member` event. The resident server which owns the provided user ID must have a valid signature on the event. If the resident server is receiving the `/send_join` request, the signature must be added before sending or persisting the event to other servers. **Added in `v1.2`** |
| `membership` | `string` | **Required:** The value `join`. |

#### Request Body Example

```json
{
  "content": {
    "membership": "join"
  },
  "origin": "matrix.org",
  "origin_server_ts": 1234567890,
  "sender": "@someone:example.org",
  "state_key": "@someone:example.org",
  "type": "m.room.member"
}
```

### Responses

| Status | Description |
| --- | --- |
| `200` | The join event has been accepted into the room. |

### 200 Response

Array of `integer, Room State`.

| Name | Type | Description |
| --- | --- | --- |
| `auth_chain` | `[PDU]` | **Required:** The auth chain for the entire current room state prior to the join event. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. |
| `state` | `[PDU]` | **Required:** The resolved current room state prior to the join event. The event format varies depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. |

#### Example

```json
[
  200,
  {
    "auth_chain": [
      {
        "content": {
          "see_room_version_spec": "The event format changes depending on the room version."
        },
        "room_id": "!somewhere:example.org",
        "type": "m.room.minimal_pdu"
      }
    ],
    "state": [
      {
        "content": {
          "see_room_version_spec": "The event format changes depending on the room version."
        },
        "room_id": "!somewhere:example.org",
        "type": "m.room.minimal_pdu"
      }
    ]
  }
]
```

## Restricted Rooms

**Added in `v1.2`**

Restricted rooms are a room type which allow federation of users from other rooms or servers. They work by showing which rooms or servers are able to issue invites via the `join_rules` event content.

When joining a restricted room, the joining server must provide evidence that the user meets the conditions required by the restricted room. This is done through the `join_authorised_via_users_server` field in the membership event.

### Join Authorization for Restricted Rooms

For restricted rooms, the following authorization process is used:

1. **Condition Validation**: The resident server checks if the joining user meets at least one of the conditions specified in the `m.room.join_rules` event.

2. **Authorization Server Selection**: If conditions are met, the resident server selects an arbitrary user ID from its own server that has appropriate permissions to issue invites.

3. **Signature Addition**: The resident server adds its signature to the join event, specifically signing the `join_authorised_via_users_server` field.

4. **Event Acceptance**: The join event is accepted with the authorization evidence included.

### Error Conditions for Restricted Rooms

- `M_UNABLE_TO_AUTHORISE_JOIN`: Returned when the server cannot validate any of the restricted room conditions (e.g., it doesn't know about any of the required rooms).

- `M_UNABLE_TO_GRANT_JOIN`: Returned when the resident server can see that the user satisfies conditions but cannot properly authorize the join (e.g., lacks appropriate signature capabilities).

## Join Rules

Room joining behavior depends on the `m.room.join_rules` event in the room state:

### Public Rooms
- **Join Rules**: `public`
- **Authorization**: Anyone can join without invitation
- **Federation**: Join requests from any server are allowed

### Invite-Only Rooms
- **Join Rules**: `invite`
- **Authorization**: Users must be explicitly invited
- **Federation**: Joining server must provide evidence of valid invitation

### Restricted Rooms
- **Join Rules**: `restricted`
- **Authorization**: Users must meet specified conditions (room membership, server membership)
- **Federation**: Complex authorization through `join_authorised_via_users_server`

### Knock Rooms
- **Join Rules**: `knock`
- **Authorization**: Users must knock and be accepted by room members
- **Federation**: Knock requests are federated to room moderators

### Knock + Restricted Rooms
- **Join Rules**: `knock_restricted`
- **Authorization**: Users can either meet restricted conditions or knock for approval
- **Federation**: Supports both restricted and knock workflows

## Join Authorization Rules

The authorization rules for room joins depend on several factors:

### Power Level Requirements
- User's power level must meet the required join level (default: 0)
- For restricted rooms, the authorizing server user must have invite permissions
- Banned users cannot join regardless of other conditions

### Membership State Transitions
- `join` → `join`: Not allowed (duplicate membership)
- `leave`/`ban` → `join`: Requires appropriate authorization
- `invite` → `join`: Accepting an invitation (simplified authorization)
- `knock` → `join`: Requires moderator approval

### Auth Event Validation
Join events must include appropriate auth events:
- `m.room.create` event
- `m.room.power_levels` event (if present)
- Sender's current `m.room.member` event (if present)
- Target's current `m.room.member` event (if present)
- `m.room.join_rules` event (for join authorization)
- `m.room.third_party_invite` event (if applicable)

## Implementation Considerations

### Server Selection
- Choose resident servers that are likely to be online and responsive
- Prefer servers that host active users in the room
- Implement fallback mechanisms if the chosen server fails

### Caching and Performance
- Cache room join capabilities to avoid repeated directory lookups
- Pre-validate join conditions where possible
- Batch process multiple join requests efficiently

### Security Considerations
- Validate all signatures on join events
- Verify restricted room conditions thoroughly
- Prevent join spam and abuse through rate limiting
- Handle malicious or invalid join attempts gracefully

### Error Handling
- Implement proper retry logic for failed joins
- Provide clear error messages to clients
- Handle network failures and server unavailability
- Support graceful degradation when services are offline