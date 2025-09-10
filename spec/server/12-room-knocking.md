# Room Knocking Federation

This document covers the federation aspects of Matrix room knocking, allowing users to request access to rooms with knock-enabled join rules.

## Overview

Rooms can permit knocking through the join rules, and if permitted this gives users a way to request to join (be invited) to the room. Users who knock on a room where the server is already a resident of the room can just send the knock event directly without using this process, however much like [joining rooms](https://spec.matrix.org/unstable/server-server-api/#joining-rooms) the server must handshake their way into having the knock sent on its behalf.

The handshake is largely the same as the joining rooms handshake, where instead of a "joining server" there is a "knocking server", and the APIs to be called are different (`/make_knock` and `/send_knock`).

Servers can retract knocks over federation by leaving the room, as described below for rejecting invites.

## GET /_matrix/federation/v1/make_knock/{roomId}/{userId}

> **Added in `v1.1`**

Asks the receiving server to return information that the sending server will need to prepare a knock event for the room.

### Request Parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room ID that is about to be knocked. |
| `userId` | `string` | **Required:** The user ID the knock event will be for. |

### Query Parameters

| Name | Type | Description |
| --- | --- | --- |
| `ver` | `[string]` | **Required:** The room versions the sending server has support for. |

### Authentication

- **Rate-limited:** No
- **Requires authentication:** Yes

### Response Codes

| Status | Description |
| --- | --- |
| `200` | A template to be used for the rest of the [Knocking Rooms](https://spec.matrix.org/unstable/server-server-api/#knocking-rooms) handshake. Note that events have a different format depending on room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. **The response body here describes the common event fields in more detail and may be missing other required fields for a PDU.** |
| `400` | The request is invalid or the room the server is attempting to knock upon has a version that is not listed in the `ver` parameters. The error should be passed through to clients so that they may give better feedback to users. |
| `403` | The knocking server or user is not permitted to knock on the room, such as when the server/user is banned or the room is not set up for receiving knocks. |
| `404` | The room that the knocking server is attempting to knock upon is unknown to the receiving server. |

### 200 Response Format

| Name | Type | Description |
| --- | --- | --- |
| `event` | `Event Template` | **Required:** An unsigned template event. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. |
| `room_version` | `string` | **Required:** The version of the room where the server is trying to knock. |

#### Event Template Structure

| Name | Type | Description |
| --- | --- | --- |
| `content` | `Membership Event Content` | **Required:** The content of the event. |
| `origin` | `string` | **Required:** The name of the resident homeserver. |
| `origin_server_ts` | `integer` | **Required:** A timestamp added by the resident homeserver. |
| `sender` | `string` | **Required:** The user ID of the knocking member. |
| `state_key` | `string` | **Required:** The user ID of the knocking member. |
| `type` | `string` | **Required:** The value `m.room.member`. |

#### Membership Event Content

| Name | Type | Description |
| --- | --- | --- |
| `membership` | `string` | **Required:** The value `knock`. |

#### Example Response

```json
{
  "event": {
    "content": {
      "membership": "knock"
    },
    "origin": "example.org",
    "origin_server_ts": 1549041175876,
    "room_id": "!somewhere:example.org",
    "sender": "@someone:example.org",
    "state_key": "@someone:example.org",
    "type": "m.room.member"
  },
  "room_version": "7"
}
```

### Error Responses

#### 400 Response Format

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |
| `room_version` | `string` | The version of the room. Required if the `errcode` is `M_INCOMPATIBLE_ROOM_VERSION`. |

```json
{
  "errcode": "M_INCOMPATIBLE_ROOM_VERSION",
  "error": "Your homeserver does not support the features required to knock on this room",
  "room_version": "7"
}
```

#### 403 Response Format

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_FORBIDDEN",
  "error": "You are not permitted to knock on this room"
}
```

#### 404 Response Format

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_NOT_FOUND",
  "error": "Unknown room"
}
```

## PUT /_matrix/federation/v1/send_knock/{roomId}/{eventId}

> **Added in `v1.1`**

Submits a signed knock event to the resident server for it to accept into the room's graph. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. **The request and response body here describe the common event fields in more detail and may be missing other required fields for a PDU.**

### Request Parameters

| Name | Type | Description |
| --- | --- | --- |
| `eventId` | `string` | **Required:** The event ID for the knock event. |
| `roomId` | `string` | **Required:** The room ID that is about to be knocked upon. |

### Authentication

- **Rate-limited:** No
- **Requires authentication:** Yes

### Request Body

| Name | Type | Description |
| --- | --- | --- |
| `content` | `Membership Event Content` | **Required:** The content of the event. |
| `origin` | `string` | **Required:** The name of the knocking homeserver. |
| `origin_server_ts` | `integer` | **Required:** A timestamp added by the knocking homeserver. |
| `sender` | `string` | **Required:** The user ID of the knocking member. |
| `state_key` | `string` | **Required:** The user ID of the knocking member. |
| `type` | `string` | **Required:** The value `m.room.member`. |

#### Membership Event Content

| Name | Type | Description |
| --- | --- | --- |
| `membership` | `string` | **Required:** The value `knock`. |

#### Request Body Example

```json
{
  "content": {
    "membership": "knock"
  },
  "origin": "example.org",
  "origin_server_ts": 1549041175876,
  "sender": "@someone:example.org",
  "state_key": "@someone:example.org",
  "type": "m.room.member"
}
```

### Response Codes

| Status | Description |
| --- | --- |
| `200` | Information about the room to pass along to clients regarding the knock. |
| `403` | The knocking server or user is not permitted to knock on the room, such as when the server/user is banned or the room is not set up for receiving knocks. |
| `404` | The room that the knocking server is attempting to knock upon is unknown to the receiving server. |

### 200 Response Format

| Name | Type | Description |
| --- | --- | --- |
| `knock_room_state` | `[StrippedStateEvent]` | **Required:** A list of [stripped state events](https://spec.matrix.org/unstable/client-server-api/#stripped-state) to help the initiator of the knock identify the room. |

#### StrippedStateEvent

| Name | Type | Description |
| --- | --- | --- |
| `content` | `EventContent` | **Required:** The `content` for the event. |
| `sender` | `string` | **Required:** The `sender` for the event. |
| `state_key` | `string` | **Required:** The `state_key` for the event. |
| `type` | `string` | **Required:** The `type` for the event. |

#### Example Response

```json
{
  "knock_room_state": [
    {
      "content": {
        "name": "Example Room"
      },
      "sender": "@bob:example.org",
      "state_key": "",
      "type": "m.room.name"
    },
    {
      "content": {
        "join_rule": "knock"
      },
      "sender": "@bob:example.org",
      "state_key": "",
      "type": "m.room.join_rules"
    }
  ]
}
```

### Error Responses

#### 403 Response Format

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_FORBIDDEN",
  "error": "You are not permitted to knock on this room"
}
```

#### 404 Response Format

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_NOT_FOUND",
  "error": "Unknown room"
}
```

## Knock Event Processing Flow

### Federation Handshake Sequence

1. **Initiate Knock**: The knocking server sends a GET request to `/make_knock` on a resident server
2. **Template Generation**: The resident server validates the request and returns an unsigned event template
3. **Event Signing**: The knocking server adds signatures and event ID to the template  
4. **Event Submission**: The knocking server sends the signed event via PUT to `/send_knock`
5. **Room State Response**: The resident server accepts the knock and returns room state information for the client

### Knock Authorization Rules

The following authorization rules apply to knock events:

- **Join Rules**: The room must have `join_rule` set to `knock` to permit knocking
- **User Status**: The user must not already be in the room or banned from the room
- **Server Permissions**: The knocking server must not be denied by server ACLs
- **Room Visibility**: The room must allow knocks from the requesting server

### Accepting Knocks

Knocks are accepted by room members with sufficient power levels sending standard invite events. When a user's knock is accepted:

1. An authorized room member creates an `m.room.member` invite event for the knocking user
2. The invite is processed through normal invitation federation flows
3. The knocking user receives the invite and can join the room normally

Note that invites are used to indicate that knocks were accepted. As such, receiving servers should be prepared to manually link up a previous knock to an invite if the invite event does not directly reference the knock.

### Retracting Knocks

Servers can retract knocks over federation by leaving the room, using the same process described for rejecting invites. This allows users to cancel their knock requests.

## Implementation Considerations

### Event Validation

Servers implementing room knocking federation must:

- Validate event structure according to room version specifications
- Verify cryptographic signatures on submitted events  
- Check authorization rules before accepting knock events
- Maintain proper event ordering in the room DAG

### State Management  

Knock events affect room state by:

- Adding the user's membership state as "knock"
- Updating the room's member list to include knocking users
- Providing room state context to knocking clients
- Enabling room members to see pending knocks

### Room State Context

The `knock_room_state` response provides essential room information to help clients:

- Display room name and topic to the knocking user
- Show room avatar and other identifying information
- Confirm join rules and room settings
- Provide context for the knock request

### Performance Optimization

For optimal performance:

- Cache room join rule information for authorization checks
- Implement efficient signature verification
- Use appropriate timeouts for federation requests
- Handle network failures gracefully with retries

### Security Considerations

Important security aspects include:

- Validating that rooms actually allow knocking before processing knock events
- Preventing knock spam through rate limiting and authorization checks
- Ensuring proper room state filtering in knock responses
- Protecting against malicious resident servers providing false room state

## Server Access Control List Protection

The following endpoints MUST be protected by Server ACLs when configured:

- `/_matrix/federation/v1/make_knock`
- `/_matrix/federation/v1/send_knock`

When a remote server makes a request, it MUST be verified to be allowed by the server ACLs. If the server is denied by the ACLs, the request should be rejected with an appropriate error response.

## Join Rules Requirements

Room knocking is only available in rooms where:

- The `join_rule` is set to `knock` in the room's `m.room.join_rules` state event
- The room version supports knocking (room versions 7 and above)
- The room has not banned the knocking user or server

Servers MUST validate these requirements before processing knock requests and MUST return appropriate error responses when requirements are not met.

## Related Specifications

- [Room Version Specifications](https://spec.matrix.org/unstable/rooms/) - Event format requirements and knocking support
- [Server-Server API Authentication](https://spec.matrix.org/unstable/server-server-api/#authentication) - Request signing  
- [Authorization Rules](https://spec.matrix.org/unstable/server-server-api/#authorization-rules) - Event validation
- [Room Join Rules](https://spec.matrix.org/unstable/client-server-api/#mroomjoin_rules) - Join rule configuration
- [Server Access Control Lists](https://spec.matrix.org/unstable/client-server-api/#server-access-control-lists-acls-for-rooms) - ACL enforcement

## Version History

Room knocking was introduced in Matrix specification version 1.1. Servers implementing knocking federation must ensure compatibility with room versions 7 and above, which include the necessary support for knock membership states and related authorization rules.