# Room Leaving Federation

This document covers the federation aspects of leaving Matrix rooms, including rejecting invites and the associated server-to-server API endpoints.

## Overview

Normally homeservers can send appropriate `m.room.member` events to have users leave the room, to reject local invites, or to retract a knock. Remote invites/knocks from other homeservers do not involve the server in the graph and therefore need another approach to reject the invite. Joining the room and promptly leaving is not recommended as clients and servers will interpret that as accepting the invite, then leaving the room rather than rejecting the invite.

Similar to the [Joining Rooms](https://spec.matrix.org/unstable/server-server-api/#joining-rooms) handshake, the server which wishes to leave the room starts with sending a `/make_leave` request to a resident server. In the case of rejecting invites, the resident server may be the server which sent the invite. After receiving a template event from `/make_leave`, the leaving server signs the event and replaces the `event_id` with its own. This is then sent to the resident server via `/send_leave`. The resident server will then send the event to other servers in the room.

## GET /_matrix/federation/v1/make_leave/{roomId}/{userId}

Asks the receiving server to return information that the sending server will need to prepare a leave event to get out of the room.

### Request Parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room ID that is about to be left. |
| `userId` | `string` | **Required:** The user ID the leave event will be for. |

### Authentication

- **Rate-limited:** No
- **Requires authentication:** Yes

### Response Codes

| Status | Description |
| --- | --- |
| `200` | A template to be used to call `/send_leave`. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. **The response body here describes the common event fields in more detail and may be missing other required fields for a PDU.** |
| `403` | The request is not authorized. This could mean that the user is not in the room. |

### 200 Response Format

| Name | Type | Description |
| --- | --- | --- |
| `event` | `Event Template` | An unsigned template event. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. |
| `room_version` | `string` | The version of the room where the server is trying to leave. If not provided, the room version is assumed to be either "1" or "2". |

#### Event Template Structure

| Name | Type | Description |
| --- | --- | --- |
| `content` | `Membership Event Content` | **Required:** The content of the event. |
| `origin` | `string` | **Required:** The name of the resident homeserver. |
| `origin_server_ts` | `integer` | **Required:** A timestamp added by the resident homeserver. |
| `sender` | `string` | **Required:** The user ID of the leaving member. |
| `state_key` | `string` | **Required:** The user ID of the leaving member. |
| `type` | `string` | **Required:** The value `m.room.member`. |

#### Membership Event Content

| Name | Type | Description |
| --- | --- | --- |
| `membership` | `string` | **Required:** The value `leave`. |

#### Example Response

```json
{
  "event": {
    "content": {
      "membership": "leave"
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

### 403 Error Response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{
  "errcode": "M_FORBIDDEN",
  "error": "User is not in the room."
}
```

## PUT /_matrix/federation/v1/send_leave/{roomId}/{eventId}

> **Note:** Servers should instead prefer to use the v2 `/send_leave` endpoint.

Submits a signed leave event to the resident server for it to accept it into the room's graph. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. **The request and response body here describe the common event fields in more detail and may be missing other required fields for a PDU.**

### Request Parameters

| Name | Type | Description |
| --- | --- | --- |
| `eventId` | `string` | **Required:** The event ID for the leave event. |
| `roomId` | `string` | **Required:** The room ID that is about to be left. |

### Authentication

- **Rate-limited:** No
- **Requires authentication:** Yes

### Request Body

| Name | Type | Description |
| --- | --- | --- |
| `content` | `Membership Event Content` | **Required:** The content of the event. |
| `depth` | `integer` | **Required:** This field must be present but is ignored; it may be 0. |
| `origin` | `string` | **Required:** The name of the leaving homeserver. |
| `origin_server_ts` | `integer` | **Required:** A timestamp added by the leaving homeserver. |
| `sender` | `string` | **Required:** The user ID of the leaving member. |
| `state_key` | `string` | **Required:** The user ID of the leaving member. |
| `type` | `string` | **Required:** The value `m.room.member`. |

#### Membership Event Content

| Name | Type | Description |
| --- | --- | --- |
| `membership` | `string` | **Required:** The value `leave`. |

#### Request Body Example

```json
{
  "content": {
    "membership": "leave"
  },
  "depth": 12,
  "origin": "matrix.org",
  "origin_server_ts": 1234567890,
  "sender": "@someone:example.org",
  "state_key": "@someone:example.org",
  "type": "m.room.member"
}
```

### Response

| Status | Description |
| --- | --- |
| `200` | An empty response to indicate the event was accepted into the graph by the receiving homeserver. |

#### 200 Response Format

Array of `integer, Empty Object`.

```json
[
  200,
  {}
]
```

## PUT /_matrix/federation/v2/send_leave/{roomId}/{eventId}

> **Note:** This API is nearly identical to the v1 API with the exception of the response format being fixed.

This endpoint is preferred over the v1 API as it provides a more standardised response format. Senders which receive a 400, 404, or other status code which indicates this endpoint is not available should retry using the v1 API instead.

Submits a signed leave event to the resident server for it to accept it into the room's graph. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. **The request and response body here describe the common event fields in more detail and may be missing other required fields for a PDU.**

### Request Parameters

| Name | Type | Description |
| --- | --- | --- |
| `eventId` | `string` | **Required:** The event ID for the leave event. |
| `roomId` | `string` | **Required:** The room ID that is about to be left. |

### Authentication

- **Rate-limited:** No
- **Requires authentication:** Yes

### Request Body

| Name | Type | Description |
| --- | --- | --- |
| `content` | `Membership Event Content` | **Required:** The content of the event. |
| `depth` | `integer` | **Required:** This field must be present but is ignored; it may be 0. |
| `origin` | `string` | **Required:** The name of the leaving homeserver. |
| `origin_server_ts` | `integer` | **Required:** A timestamp added by the leaving homeserver. |
| `sender` | `string` | **Required:** The user ID of the leaving member. |
| `state_key` | `string` | **Required:** The user ID of the leaving member. |
| `type` | `string` | **Required:** The value `m.room.member`. |

#### Membership Event Content

| Name | Type | Description |
| --- | --- | --- |
| `membership` | `string` | **Required:** The value `leave`. |

#### Request Body Example

```json
{
  "content": {
    "membership": "leave"
  },
  "depth": 0,
  "origin": "example.org",
  "origin_server_ts": 1549041175876,
  "sender": "@someone:example.org",
  "state_key": "@someone:example.org",
  "type": "m.room.member"
}
```

### Response

| Status | Description |
| --- | --- |
| `200` | An empty response to indicate the event was accepted into the graph by the receiving homeserver. |

#### 200 Response Format

```json
{}
```

## Leave Event Processing Flow

### Federation Handshake Sequence

1. **Initiate Leave**: The leaving server sends a GET request to `/make_leave` on a resident server
2. **Template Generation**: The resident server validates the request and returns an unsigned event template
3. **Event Signing**: The leaving server adds signatures and event ID to the template  
4. **Event Submission**: The leaving server sends the signed event via PUT to `/send_leave`
5. **Event Propagation**: The resident server accepts the event and propagates it to other servers

### Authorization Rules

The following authorization rules apply to leave events:

- **User Membership**: The user must be in the room (joined or invited) to leave
- **Invite Rejection**: Users can reject invites by leaving without joining first  
- **Power Levels**: No special power levels are required to leave a room
- **Ban Enforcement**: Banned users cannot leave (they are already removed)

### Error Handling

Common error scenarios include:

- **M_FORBIDDEN**: User is not in the room or not authorized to leave
- **M_NOT_FOUND**: Room or user does not exist
- **M_INVALID_SIGNATURE**: Event signature validation failed
- **M_BAD_JSON**: Malformed event structure

## Server Access Control List Protection

The following endpoints MUST be protected by Server ACLs when configured:

- `/_matrix/federation/v1/make_leave`  
- `/_matrix/federation/v1/send_leave`
- `/_matrix/federation/v2/send_leave`

When a remote server makes a request, it MUST be verified to be allowed by the server ACLs. If the server is denied by the ACLs, the request should be rejected with an appropriate error response.

## Implementation Considerations

### Event Validation

Servers implementing room leave federation must:

- Validate event structure according to room version specifications
- Verify cryptographic signatures on submitted events
- Check authorization rules before accepting leave events
- Maintain proper event ordering in the room DAG

### State Management  

Leave events affect room state by:

- Removing the user's membership state
- Updating the room's member list  
- Potentially affecting power level calculations
- Triggering state resolution if conflicts occur

### Performance Optimization

For optimal performance:

- Cache resident server information for rooms
- Implement efficient signature verification
- Use appropriate timeouts for federation requests
- Handle network failures gracefully with retries

### Security Considerations

Important security aspects include:

- Validating that users can only leave rooms they're actually in
- Preventing replay attacks through proper event ID generation  
- Ensuring proper authorization before processing leave events
- Protecting against malicious resident servers

## Related Specifications

- [Room Version Specifications](https://spec.matrix.org/unstable/rooms/) - Event format requirements
- [Server-Server API Authentication](https://spec.matrix.org/unstable/server-server-api/#authentication) - Request signing
- [Authorization Rules](https://spec.matrix.org/unstable/server-server-api/#authorization-rules) - Event validation
- [Server Access Control Lists](https://spec.matrix.org/unstable/client-server-api/#server-access-control-lists-acls-for-rooms) - ACL enforcement