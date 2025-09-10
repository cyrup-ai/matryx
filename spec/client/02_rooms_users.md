# Matrix Client-Server API - Rooms and Users Management

This section covers the Matrix Client-Server API endpoints and concepts related to room creation, membership management, room aliases, and user permissions.

## Room Creation

- `m.room.power_levels`: Sets the power levels of users and required power levels for various actions in the room. This is a state event.
- `m.room.join_rules`: Whether the room is "invite-only" or not.

See [Room Events](https://spec.matrix.org/unstable/client-server-api/#room-events) for more information on these events. To create a room, a client has to use the following API.

## POST /_matrix/client/v3/createRoom

---

**Changed in `v1.16`:** Added server behaviour for how the initial power levels change depending on room version.

Create a new room with various configuration options.

The server MUST apply the normal state resolution rules when creating the new room, including checking power levels for each event. It MUST apply the events implied by the request in the following order:

1. The `m.room.create` event itself. Must be the first event in the room.
2. An `m.room.member` event for the creator to join the room. This is needed so the remaining events can be sent.
3. A default `m.room.power_levels` event. Overridden by the `power_level_content_override` parameter.
	In [room versions](https://spec.matrix.org/unstable/rooms/) 1 through 11, the room creator (and not other members) will be given permission to send state events.
	In room versions 12 and later, the room creator is given infinite power level and cannot be specified in the `users` field of `m.room.power_levels`, so is not listed explicitly.
	**Note**: For `trusted_private_chat`, the users specified in the `invite` parameter SHOULD also be appended to `additional_creators` by the server, per the `creation_content` parameter.
	If the room's version is 12 or higher, the power level for sending `m.room.tombstone` events MUST explicitly be higher than `state_default`. For example, set to 150 instead of 100.
4. An `m.room.canonical_alias` event if `room_alias_name` is given.
5. Events set by the `preset`. Currently these are the `m.room.join_rules`,`m.room.history_visibility`, and `m.room.guest_access` state events.
6. Events listed in `initial_state`, in the order that they are listed.
7. Events implied by `name` and `topic` (`m.room.name` and `m.room.topic` state events).
8. Invite events implied by `invite` and `invite_3pid` (`m.room.member` with `membership: invite` and `m.room.third_party_invite`).

The available presets do the following with respect to room state:

| Preset | `join_rules` | `history_visibility` | `guest_access` | Other |
| --- | --- | --- | --- | --- |
| `private_chat` | `invite` | `shared` | `can_join` |  |
| `trusted_private_chat` | `invite` | `shared` | `can_join` | All invitees are given the same power level as the room creator. |
| `public_chat` | `public` | `shared` | `forbidden` |  |

The server will create a `m.room.create` event in the room with the requesting user as the creator, alongside other keys provided in the `creation_content` or implied by behaviour of `creation_content`.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

### Request

#### Request body

| Name | Type | Description |
| --- | --- | --- |
| `creation_content` | `CreationContent` | Extra keys, such as `m.federate`, to be added to the content of the [`m.room.create`](https://spec.matrix.org/unstable/client-server-api/#mroomcreate) event.  The server will overwrite the following keys: `creator`, `room_version`. Future versions of the specification may allow the server to overwrite other keys.  When using the `trusted_private_chat` preset, the server SHOULD combine `additional_creators` specified here and the `invite` array into the eventual `m.room.create` event's `additional_creators`, deduplicating between the two parameters.  **Changed in `v1.16`:** Added server behaviour for how to handle `trusted_private_chat` and invited users. |
| `initial_state` | `[[StateEvent](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3createroom_request_stateevent)]` | A list of state events to set in the new room. This allows the user to override the default state events set in the new room. The expected format of the state events are an object with type, state\_key and content keys set.  Takes precedence over events set by `preset`, but gets overridden by `name` and `topic` keys. |
| `invite` | `[string]` | A list of user IDs to invite to the room. This will tell the server to invite everyone in the list to the newly created room. |
| `invite_3pid` | `[[Invite3pid](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3createroom_request_invite3pid)]` | A list of objects representing third-party IDs to invite into the room. |
| `is_direct` | `boolean` | This flag makes the server set the `is_direct` flag on the `m.room.member` events sent to the users in `invite` and `invite_3pid`. See [Direct Messaging](https://spec.matrix.org/unstable/client-server-api/#direct-messaging) for more information. |
| `name` | `string` | If this is included, an [`m.room.name`](https://spec.matrix.org/unstable/client-server-api/#mroomname) event will be sent into the room to indicate the name for the room. This overwrites any [`m.room.name`](https://spec.matrix.org/unstable/client-server-api/#mroomname) event in `initial_state`. |
| `power_level_content_override` | `Power Level Event Content` | The power level content to override in the default power level event. This object is applied on top of the generated [`m.room.power_levels`](https://spec.matrix.org/unstable/client-server-api/#mroompower_levels) event content prior to it being sent to the room. Defaults to overriding nothing. |
| `preset` | `string` | Convenience parameter for setting various default state events based on a preset.  If unspecified, the server should use the `visibility` to determine which preset to use. A visibility of `public` equates to a preset of `public_chat` and `private` visibility equates to a preset of `private_chat`.  One of: `[private_chat, public_chat, trusted_private_chat]`. |
| `room_alias_name` | `string` | The desired room alias **local part**. If this is included, a room alias will be created and mapped to the newly created room. The alias will belong on the *same* homeserver which created the room. For example, if this was set to "foo" and sent to the homeserver "example.com" the complete room alias would be `#foo:example.com`.  The complete room alias will become the canonical alias for the room and an `m.room.canonical_alias` event will be sent into the room. |
| `room_version` | `string` | The room version to set for the room. If not provided, the homeserver is to use its configured default. If provided, the homeserver will return a 400 error with the errcode `M_UNSUPPORTED_ROOM_VERSION` if it does not support the room version. |
| `topic` | `string` | If this is included, an [`m.room.topic`](https://spec.matrix.org/unstable/client-server-api/#mroomtopic) event with a `text/plain` mimetype will be sent into the room to indicate the topic for the room. This overwrites any [`m.room.topic`](https://spec.matrix.org/unstable/client-server-api/#mroomtopic) event in `initial_state`. |
| `visibility` | `string` | The room's visibility in the server's [published room directory](https://spec.matrix.org/unstable/client-server-api/#published-room-directory). Defaults to `private`.  One of: `[public, private]`. |

#### StateEvent

| Name | Type | Description |
| --- | --- | --- |
| `content` | `object` | **Required:** The content of the event. |
| `state_key` | `string` | The state\_key of the state event. Defaults to an empty string. |
| `type` | `string` | **Required:** The type of event to send. |

#### Invite3pid

| Name | Type | Description |
| --- | --- | --- |
| `address` | `string` | **Required:** The invitee's third-party identifier. |
| `id_access_token` | `string` | **Required:** An access token previously registered with the identity server. Servers can treat this as optional to distinguish between r0.5-compatible clients and this specification version. |
| `id_server` | `string` | **Required:** The hostname+port of the identity server which should be used for third-party identifier lookups. |
| `medium` | `string` | **Required:** The kind of address being passed in the address field, for example `email` (see [the list of recognised values](https://spec.matrix.org/unstable/appendices/#3pid-types)). |

#### Request body example

```json
{
  "creation_content": {
    "m.federate": false
  },
  "name": "The Grand Duke Pub",
  "preset": "public_chat",
  "room_alias_name": "thepub",
  "topic": "All about happy hour"
}
```

### Responses

| Status | Description |
| --- | --- |
| `200` | Information about the newly created room. |
| `400` | The request is invalid. A meaningful `errcode` and description error text will be returned. Example reasons for rejection include:  - The request body is malformed (`errcode` set to `M_BAD_JSON` or `M_NOT_JSON`). - The room alias specified is already taken (`errcode` set to `M_ROOM_IN_USE`). - The initial state implied by the parameters to the request is invalid: for example, the user's `power_level` is set below that necessary to set the room name (`errcode` set to `M_INVALID_ROOM_STATE`). - The homeserver doesn't support the requested room version, or one or more users being invited to the new room are residents of a homeserver which does not support the requested room version. The `errcode` will be `M_UNSUPPORTED_ROOM_VERSION` in these cases. |

#### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `room_id` | `string` | **Required:** The created room's ID. |

```json
{
  "room_id": "!sefiuhWgwghwWgh:example.com"
}
```

#### 400 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_UNKNOWN",
  "error": "An unknown error occurred"
}
```

## Room Aliases

Servers may host aliases for rooms with human-friendly names. Aliases take the form `#friendlyname:server.name`.

As room aliases are scoped to a particular homeserver domain name, it is likely that a homeserver will reject attempts to maintain aliases on other domain names. This specification does not provide a way for homeservers to send update requests to other servers. However, homeservers MUST handle `GET` requests to resolve aliases on other servers; they should do this using the federation API if necessary.

Rooms do not store a list of all aliases present on a room, though members of the room with relevant permissions may publish preferred aliases through the `m.room.canonical_alias` state event. The aliases in the state event should point to the room ID they are published within, however room aliases can and do drift to other room IDs over time. Clients SHOULD NOT treat the aliases as accurate. They SHOULD be checked before they are used or shared with another user. If a room appears to have a room alias of `#alias:example.com`, this SHOULD be checked to make sure that the room's ID matches the `room_id` returned from the request.

## GET /_matrix/client/v3/directory/room/{roomAlias}

---

Requests that the server resolve a room alias to a room ID.

The server will use the federation API to resolve the alias if the domain part of the alias does not correspond to the server's own domain.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | No |

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomAlias` | `string` | **Required:** The room alias. Its format is defined [in the appendices](https://spec.matrix.org/unstable/appendices/#room-aliases). |

### Responses

| Status | Description |
| --- | --- |
| `200` | The room ID and other information for this alias. |
| `400` | The given `roomAlias` is not a valid room alias. |
| `404` | There is no mapped room ID for this room alias. |

#### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `room_id` | `string` | The room ID for this room alias. |
| `servers` | `[string]` | A list of servers that are aware of this room alias. |

```json
{
  "room_id": "!abnjk1jdasj98:capuchins.com",
  "servers": [
    "capuchins.com",
    "matrix.org",
    "another.com"
  ]
}
```

#### 400 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_INVALID_PARAM",
  "error": "Room alias invalid"
}
```

#### 404 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_NOT_FOUND",
  "error": "Room alias #monkeys:matrix.org not found."
}
```

## PUT /_matrix/client/v3/directory/room/{roomAlias}

---

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomAlias` | `string` | **Required:** The room alias to set. Its format is defined [in the appendices](https://spec.matrix.org/unstable/appendices/#room-aliases). |

#### Request body

| Name | Type | Description |
| --- | --- | --- |
| `room_id` | `string` | **Required:** The room ID to set. |

#### Request body example

```json
{
  "room_id": "!abnjk1jdasj98:capuchins.com"
}
```

### Responses

| Status | Description |
| --- | --- |
| `200` | The mapping was created. |
| `400` | The given `roomAlias` is not a valid room alias. |
| `409` | A room alias with that name already exists. |

#### 200 response

```json
{}
```

#### 400 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_INVALID_PARAM",
  "error": "Room alias invalid"
}
```

#### 409 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_UNKNOWN",
  "error": "Room alias #monkeys:matrix.org already exists."
}
```

## DELETE /_matrix/client/v3/directory/room/{roomAlias}

---

Remove a mapping of room alias to room ID.

Servers may choose to implement additional access control checks here, for instance that room aliases can only be deleted by their creator or a server administrator.

**Note:**Servers may choose to update the `alt_aliases` for the `m.room.canonical_alias` state event in the room when an alias is removed. Servers which choose to update the canonical alias event are recommended to, in addition to their other relevant permission checks, delete the alias and return a successful response even if the user does not have permission to update the `m.room.canonical_alias` event.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomAlias` | `string` | **Required:** The room alias to remove. Its format is defined [in the appendices](https://spec.matrix.org/unstable/appendices/#room-aliases). |

### Responses

| Status | Description |
| --- | --- |
| `200` | The mapping was deleted. |
| `404` | There is no mapped room ID for this room alias. |

#### 200 response

```json
{}
```

#### 404 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_NOT_FOUND",
  "error": "Room alias #monkeys:example.org not found."
}
```

## GET /_matrix/client/v3/rooms/{roomId}/aliases

---

Get a list of aliases maintained by the local server for the given room.

This endpoint can be called by users who are in the room (external users receive an `M_FORBIDDEN` error response). If the room's `m.room.history_visibility` maps to `world_readable`, any user can call this endpoint.

Servers may choose to implement additional access control checks here, such as allowing server administrators to view aliases regardless of membership.

**Note:**Clients are recommended not to display this list of aliases prominently as they are not curated, unlike those listed in the `m.room.canonical_alias` state event.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room ID to find local aliases of. |

### Responses

| Status | Description |
| --- | --- |
| `200` | The list of local aliases for the room. |
| `400` | The given `roomAlias` is not a valid room alias. |
| `403` | The user is not permitted to retrieve the list of local aliases for the room. |
| `429` | This request was rate-limited. |

#### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `aliases` | `[string]` | **Required:** The server's local aliases on the room. Can be empty. |

```json
{
  "aliases": [
    "#somewhere:example.com",
    "#another:example.com",
    "#hat_trick:example.com"
  ]
}
```

#### 400 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_INVALID_PARAM",
  "error": "Room alias invalid"
}
```

#### 403 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_FORBIDDEN",
  "error": "You are not a member of the room."
}
```

#### 429 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** The M\_LIMIT\_EXCEEDED error code |
| `error` | `string` | A human-readable error message. |
| `retry_after_ms` | `integer` | The amount of time in milliseconds the client should wait before trying the request again. |

```json
{
  "errcode": "M_LIMIT_EXCEEDED",
  "error": "Too many requests",
  "retry_after_ms": 2000
}
```

## Permissions

**\[Changed in `v1.16`\]** Updated section to discuss creator power level in room version 12 and beyond.

Permissions for rooms are done via the concept of power levels - to do any action in a room a user must have a suitable power level. Power levels are stored as state events in a given room. The power levels required for operations and the power levels assigned to specific users are defined in the `m.room.power_levels` state event. The `m.room.power_levels` state event additionally defines some defaults, though room creators are special in that:

- In [room versions](https://spec.matrix.org/unstable/rooms/) 1 through 11, room creators by default have power level 100 but still can have that level changed by power level events, by the same rules as other members.
- In [room version 12](https://spec.matrix.org/unstable/rooms/v12/) (and beyond), room creators are *not* specified in the power levels event and have an infinitely high power level that is immutable. After room creation, users cannot be given this same infinitely high power level.

Users can grant other users increased power levels up to their own power level (or the maximum allowable integer for the room when their power level is infinitely high). For example, user A with a power level of 50 could increase the power level of user B to a maximum of level 50. Power levels for users are tracked per-room even if the user is not present in the room. The keys contained in `m.room.power_levels` determine the levels required for certain operations such as kicking, banning, and sending state events. See [`m.room.power_levels`](https://spec.matrix.org/unstable/client-server-api/#mroompower_levels) for more information.

Clients may wish to assign names to particular power levels. Most rooms will use the default power level hierarchy assigned during room creation, but rooms may still deviate slightly.

A suggested mapping is as follows:

- 0 to `state_default-1` (typically 49): User
- `state_default` to the level required to send `m.room.power_levels` events minus 1 (typically 99): Moderator
- The level required send `m.room.power_levels` events and above: Administrator
- Creators of the room, in room version 12 and beyond: Creator

Clients may also wish to distinguish "above admin" power levels based on the level required to send `m.room.tombstone` events.

## Room Membership

Users need to be a member of a room in order to send and receive events in that room. There are several states in which a user may be, in relation to a room:

- Unrelated (the user cannot send or receive events in the room)
- Knocking (the user has requested to participate in the room, but has not yet been allowed to)
- Invited (the user has been invited to participate in the room, but is not yet participating)
- Joined (the user can send and receive events in the room)
- Banned (the user is not allowed to join the room)

There are a few notable exceptions which allow non-joined members of the room to send events in the room:

- Users wishing to reject an invite would send `m.room.member` events with `content.membership` of `leave`. They must have been invited first.
- If the room allows, users can send `m.room.member` events with `content.membership` of `knock` to knock on the room. This is a request for an invite by the user.
- To retract a previous knock, a user would send a `leave` event similar to rejecting an invite.

Some rooms require that users be invited to it before they can join; others allow anyone to join. Whether a given room is an "invite-only" room is determined by the room config key `m.room.join_rules`. It can have one of the following values:

`public` This room is free for anyone to join without an invite.

`invite` This room can only be joined if you were invited.

`knock` This room can only be joined if you were invited, and allows anyone to request an invite to the room. Note that this join rule is only available in room versions [which support knocking](https://spec.matrix.org/unstable/rooms/#feature-matrix).

**\[Added in `v1.2`\]** `restricted` This room can be joined if you were invited or if you are a member of another room listed in the join rules. If the server cannot verify membership for any of the listed rooms then you can only join with an invite. Note that this rule is only expected to work in room versions [which support it](https://spec.matrix.org/unstable/rooms/#feature-matrix).

**\[Added in `v1.3`\]** `knock_restricted` This room can be joined as though it was `restricted` *or* `knock`. If you interact with the room using knocking, the `knock` rule takes effect whereas trying to join the room without an invite applies the `restricted` join rule. Note that this rule is only expected to work in room versions [which support it](https://spec.matrix.org/unstable/rooms/#feature-matrix).

The allowable state transitions of membership are:

![Diagram presenting the possible membership state transitions](https://spec.matrix.org/unstable/diagrams/membership_hu10446216246341434891.webp)

## GET /_matrix/client/v3/joined_rooms

---

This API returns a list of the user's current rooms.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

### Request

No request parameters or request body.

### Responses

| Status | Description |
| --- | --- |
| `200` | A list of the rooms the user is in. |

#### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `joined_rooms` | `[string]` | **Required:** The ID of each room in which the user has `joined` membership. |

```json
{
  "joined_rooms": [
    "!foo:example.com"
  ]
}
```

## Joining Rooms

## POST /_matrix/client/v3/rooms/{roomId}/invite

---

*Note that there are two forms of this API, which are documented separately. This version of the API requires that the inviter knows the Matrix identifier of the invitee. The other is documented in the [third-party invites](https://spec.matrix.org/unstable/client-server-api/#third-party-invites) section.*

This API invites a user to participate in a particular room. They do not start participating in the room until they actually join the room.

Only users currently in a particular room can invite other users to join that room.

If the user was invited to the room, the homeserver will append a `m.room.member` event to the room.

| Rate-limited: | Yes |### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room identifier (not alias) to which to invite the user. |

#### Request body

| Name | Type | Description |
| --- | --- | --- |
| `reason` | `string` | Optional reason to be included as the `reason` on the subsequent membership event.  **Added in `v1.1`** |
| `user_id` | `string` | **Required:** The fully qualified user ID of the invitee. |

#### Request body example

```json
{
  "reason": "Welcome to the team!",
  "user_id": "@cheeky_monkey:matrix.org"
}
```

### Responses

| Status | Description |
| --- | --- |
| `200` | The user has been invited to join the room, or was already invited to the room. |
| `400` | The request is invalid. A meaningful `errcode` and description error text will be returned. Example reasons for rejection include:  - The request body is malformed (`errcode` set to `M_BAD_JSON` or `M_NOT_JSON`). - One or more users being invited to the room are residents of a homeserver which does not support the requested room version. The `errcode` will be `M_UNSUPPORTED_ROOM_VERSION` in these cases. |
| `403` | You do not have permission to invite the user to the room. A meaningful `errcode` and description error text will be returned. Example reasons for rejections are:  - The invitee has been banned from the room. - The invitee is already a member of the room. - The inviter is not currently in the room. - The inviter's power level is insufficient to invite users to the room. |
| `429` | This request was rate-limited. |

#### 200 response

```json
{}
```

#### 400 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_UNKNOWN",
  "error": "An unknown error occurred"
}
```

#### 403 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_FORBIDDEN",
  "error": "@cheeky_monkey:matrix.org is banned from the room"
}
```

#### 429 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** The M\_LIMIT\_EXCEEDED error code |
| `error` | `string` | A human-readable error message. |
| `retry_after_ms` | `integer` | The amount of time in milliseconds the client should wait before trying the request again. |

```json
{
  "errcode": "M_LIMIT_EXCEEDED",
  "error": "Too many requests",
  "retry_after_ms": 2000
}
```

## POST /_matrix/client/v3/join/{roomIdOrAlias}

---

*Note that this API takes either a room ID or alias, unlike* `/rooms/{roomId}/join`.

This API starts a user's participation in a particular room, if that user is allowed to participate in that room. After this call, the client is allowed to see all current state events in the room, and all subsequent events associated with the room until the user leaves the room.

After a user has joined a room, the room will appear as an entry in the response of the [`/initialSync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3initialsync) and [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync) APIs.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomIdOrAlias` | `string` | **Required:** The room identifier or alias to join. |

#### Query parameters

| Name | Type | Description |
| --- | --- | --- |
| `via` | `[string]` | The servers to attempt to join the room through. One of the servers must be participating in the room.  **Added in `v1.12`** |

#### Request body

| Name | Type | Description |
| --- | --- | --- |
| `reason` | `string` | Optional reason to be included as the `reason` on the subsequent membership event.  **Added in `v1.1`** |
| `third_party_signed` | `[Third-party Signed](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3joinroomidoralias_request_third-party-signed)` | If a `third_party_signed` was supplied, the homeserver must verify that it matches a pending `m.room.third_party_invite` event in the room, and perform key validity checking if required by the event. |

##### Third-party Signed

| Name | Type | Description |
| --- | --- | --- |
| `mxid` | `string` | **Required:** The Matrix ID of the invitee. |
| `sender` | `string` | **Required:** The Matrix ID of the user who issued the invite. |
| `signatures` | `{string: {string: string}}` | **Required:** A signatures object containing a signature of the entire signed object. |
| `token` | `string` | **Required:** The state key of the m.third\_party\_invite event. |

#### Request body example

```json
{
  "reason": "Looking for support",
  "third_party_signed": {
    "mxid": "@bob:example.org",
    "sender": "@alice:example.org",
    "signatures": {
      "example.org": {
        "ed25519:0": "some9signature"
      }
    },
    "token": "random8nonce"
  }
}
```

### Responses

| Status | Description |
| --- | --- |
| `200` | The room has been joined.  The joined room ID must be returned in the `room_id` field. |
| `403` | You do not have permission to join the room. A meaningful `errcode` and description error text will be returned. Example reasons for rejection are:  - The room is invite-only and the user was not invited. - The user has been banned from the room. - The room is restricted and the user failed to satisfy any of the conditions. |
| `429` | This request was rate-limited. |

#### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `room_id` | `string` | **Required:** The joined room ID. |

```json
{
  "room_id": "!d41d8cd:matrix.org"
}
```

#### 403 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_FORBIDDEN",
  "error": "You are not invited to this room."
}
```

#### 429 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** The M\_LIMIT\_EXCEEDED error code |
| `error` | `string` | A human-readable error message. |
| `retry_after_ms` | `integer` | The amount of time in milliseconds the client should wait before trying the request again. |

```json
{
  "errcode": "M_LIMIT_EXCEEDED",
  "error": "Too many requests",
  "retry_after_ms": 2000
}
```

## POST /_matrix/client/v3/rooms/{roomId}/join

---

*Note that this API requires a room ID, not alias.*`/join/{roomIdOrAlias}` *exists if you have a room alias.*

This API starts a user's participation in a particular room, if that user is allowed to participate in that room. After this call, the client is allowed to see all current state events in the room, and all subsequent events associated with the room until the user leaves the room.

After a user has joined a room, the room will appear as an entry in the response of the [`/initialSync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3initialsync) and [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync) APIs.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room identifier (not alias) to join. |

#### Request body

| Name | Type | Description |
| --- | --- | --- |
| `reason` | `string` | Optional reason to be included as the `reason` on the subsequent membership event.  **Added in `v1.1`** |
| `third_party_signed` | `[Third-party Signed](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3roomsroomidjoin_request_third-party-signed)` | If supplied, the homeserver must verify that it matches a pending `m.room.third_party_invite` event in the room, and perform key validity checking if required by the event. |

##### Third-party Signed

| Name | Type | Description |
| --- | --- | --- |
| `mxid` | `string` | **Required:** The Matrix ID of the invitee. |
| `sender` | `string` | **Required:** The Matrix ID of the user who issued the invite. |
| `signatures` | `{string: {string: string}}` | **Required:** A signatures object containing a signature of the entire signed object. |
| `token` | `string` | **Required:** The state key of the m.third\_party\_invite event. |

#### Request body example

```json
{
  "reason": "Looking for support",
  "third_party_signed": {
    "mxid": "@bob:example.org",
    "sender": "@alice:example.org",
    "signatures": {
      "example.org": {
        "ed25519:0": "some9signature"
      }
    },
    "token": "random8nonce"
  }
}
```

### Responses

| Status | Description |
| --- | --- |
| `200` | The room has been joined.  The joined room ID must be returned in the `room_id` field. |
| `403` | You do not have permission to join the room. A meaningful `errcode` and description error text will be returned. Example reasons for rejection are:  - The room is invite-only and the user was not invited. - The user has been banned from the room. - The room is restricted and the user failed to satisfy any of the conditions. |
| `429` | This request was rate-limited. |

#### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `room_id` | `string` | **Required:** The joined room ID. |

```json
{
  "room_id": "!d41d8cd:matrix.org"
}
```

#### 403 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_FORBIDDEN",
  "error": "You are not invited to this room."
}
```

#### 429 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** The M\_LIMIT\_EXCEEDED error code |
| `error` | `string` | A human-readable error message. |
| `retry_after_ms` | `integer` | The amount of time in milliseconds the client should wait before trying the request again. |

```json
{
  "errcode": "M_LIMIT_EXCEEDED",
  "error": "Too many requests",
  "retry_after_ms": 2000
}
```

## Knocking on Rooms

**\[Added in `v1.1`\]** **\[Changed in `v1.3`\]**

If the join rules allow, external users to the room can `/knock` on it to request permission to join. Users with appropriate permissions within the room can then approve ([`/invite`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3roomsroomidinvite)) or deny ([`/kick`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3roomsroomidkick), [`/ban`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3roomsroomidban), or otherwise set membership to `leave`) the knock. Knocks can be retracted by calling [`/leave`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3roomsroomidleave) or otherwise setting membership to `leave`.

Users who are currently in the room, already invited, or banned cannot knock on the room.

To accept another user's knock, the user must have permission to invite users to the room. To reject another user's knock, the user must have permission to either kick or ban users (whichever is being performed). Note that setting another user's membership to `leave` is kicking them.

The knocking homeserver should assume that an invite to the room means that the knock was accepted, even if the invite is not explicitly related to the knock.

Homeservers are permitted to automatically accept invites as a result of knocks as they should be aware of the user's intent to join the room. If the homeserver is not auto-accepting invites (or there was an unrecoverable problem with accepting it), the invite is expected to be passed down normally to the client to handle. Clients can expect to see the join event if the server chose to auto-accept.

## POST /_matrix/client/v3/knock/{roomIdOrAlias}

---

**Added in `v1.1`**

*Note that this API takes either a room ID or alias, unlike other membership APIs.*

This API "knocks" on the room to ask for permission to join, if the user is allowed to knock on the room. Acceptance of the knock happens out of band from this API, meaning that the client will have to watch for updates regarding the acceptance/rejection of the knock.

If the room history settings allow, the user will still be able to see history of the room while being in the "knock" state. The user will have to accept the invitation to join the room (acceptance of knock) to see messages reliably. See the `/join` endpoints for more information about history visibility to the user.

The knock will appear as an entry in the response of the [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync) API.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomIdOrAlias` | `string` | **Required:** The room identifier or alias to knock upon. |

#### Query parameters

| Name | Type | Description |
| --- | --- | --- |
| `via` | `[string]` | The servers to attempt to knock on the room through. One of the servers must be participating in the room.  **Added in `v1.12`** |

#### Request body

| Name | Type | Description |
| --- | --- | --- |
| `reason` | `string` | Optional reason to be included as the `reason` on the subsequent membership event. |

#### Request body example

```json
{
  "reason": "Looking for support"
}
```

### Responses

| Status | Description |
| --- | --- |
| `200` | The room has been knocked upon.  The knocked room ID must be returned in the `room_id` field. |
| `403` | You do not have permission to knock on the room. A meaningful `errcode` and description error text will be returned. Example reasons for rejection are:  - The room is not set up for knocking. - The user has been banned from the room. |
| `404` | The room could not be found or resolved to a room ID. |
| `429` | This request was rate-limited. |

#### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `room_id` | `string` | **Required:** The knocked room ID. |

```json
{
  "room_id": "!d41d8cd:matrix.org"
}
```

#### 403 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_FORBIDDEN",
  "error": "You are not allowed to knock on this room."
}
```

#### 404 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_NOT_FOUND",
  "error": "That room does not appear to exist."
}
```

#### 429 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** The M\_LIMIT\_EXCEEDED error code |
| `error` | `string` | A human-readable error message. |
| `retry_after_ms` | `integer` | The amount of time in milliseconds the client should wait before trying the request again. |

```json
{
  "errcode": "M_LIMIT_EXCEEDED",
  "error": "Too many requests",
  "retry_after_ms": 2000
}