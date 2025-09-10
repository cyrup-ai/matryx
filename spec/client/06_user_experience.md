---
title: "Matrix Client-Server API: User Experience"
description: "User experience and integration features including client configuration, SSO, content reporting, and administrative functions"
---

# User Experience

*This file contains content from approximately lines 25587-28736 of the original Matrix Client-Server API specification, covering user experience and integration functionality.*

## Table of Contents

- [Room Tagging](#room-tagging)
- [Client Configuration](#client-configuration)
- [Server Administration](#server-administration)
- [Event Context](#event-context)
- [SSO Authentication](#sso-authentication)
- [Direct Messaging](#direct-messaging)
- [Ignoring Users](#ignoring-users)
- [Sticker Messages](#sticker-messages)
- [Content Reporting](#content-reporting)
- [Third-party Networks](#third-party-networks)
- [OpenID Integration](#openid-integration)
- [Server ACLs](#server-acls)
- [User/Room Mentions](#userroom-mentions)
- [Room Upgrades](#room-upgrades)
- [Server Notices](#server-notices)
- [Moderation Policy Lists](#moderation-policy-lists)

## Room Tagging

Users can add tags to rooms. Tags are namespaced strings used to label rooms. A room may have multiple tags. Tags are only visible to the user that set them but are shared across all their devices.

### Events

The tags on a room are received as single `m.tag` event in the `account_data` section of a room. The content of the `m.tag` event is a `tags` key whose value is an object mapping the name of each tag to another object.

The JSON object associated with each tag gives information about the tag, e.g how to order the rooms with a given tag.

Ordering information is given under the `order` key as a number between 0 and 1. The numbers are compared such that 0 is displayed first. Therefore a room with an `order` of `0.2` would be displayed before a room with an `order` of `0.7`. If a room has a tag without an `order` key then it should appear after the rooms with that tag that have an `order` key.

The name of a tag MUST NOT exceed 255 bytes.

The tag namespace is defined as follows:

- The namespace `m.*` is reserved for tags defined in the Matrix specification. Clients must ignore any tags in this namespace they don't understand.
- The namespace `u.*` is reserved for user-defined tags. The portion of the string after the `u.` is defined to be the display name of this tag. No other semantics should be inferred from tags in this namespace.
- A client or app willing to use special tags for advanced functionality should namespace them similarly to state keys:`tld.name.*`
- Any tag in the `tld.name.*` form but not matching the namespace of the current client should be ignored
- Any tag not matching the above rules should be interpreted as a user tag from the `u.*` namespace, as if the name had already had `u.`stripped from the start (ie. the name of the tag is used as the display name directly). These non-namespaced tags are supported for historical reasons. New tags should use one of the defined namespaces above.

Several special names are listed in the specification: The following tags are defined in the `m.*` namespace:

- `m.favourite`: The user's favourite rooms. These should be shown with higher precedence than other rooms.
- `m.lowpriority`: These should be shown with lower precedence than others.
- `m.server_notice`: Used to identify [Server Notice Rooms](https://spec.matrix.org/unstable/client-server-api/#server-notices).

## m.tag

---

Informs the client of tags on a room.

| Event type: | Message event |
| --- | --- |

## Content

| Name | Type | Description |
| --- | --- | --- |
| `tags` | `{string: [Tag](https://spec.matrix.org/unstable/client-server-api/#mtag_tag)}` | The tags on the room and their contents. |

| Name | Type | Description |
| --- | --- | --- |
| `order` | `number` | A number in a range `[0,1]` describing a relative position of the room under the given tag. |

## Examples

```json
{

  "content": {

    "tags": {

      "u.work": {

        "order": 0.9

      }

    }

  },

  "type": "m.tag"

}
```

### Client Behaviour

## GET /\_matrix/client/v3/user/{userId}/rooms/{roomId}/tags

---

List the tags set by a user on a room.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The ID of the room to get tags for. |
| `userId` | `string` | **Required:** The id of the user to get tags for. The access token must be authorized to make requests for this user ID. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The list of tags for the user for the room. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `tags` | `{string: [Tag](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3useruseridroomsroomidtags_response-200_tag)}` |  |

| Name | Type | Description |
| --- | --- | --- |
| `order` | `number` | A number in a range `[0,1]` describing a relative position of the room under the given tag. |

```json
{

  "tags": {

    "m.favourite": {

      "order": 0.1

    },

    "u.Customers": {},

    "u.Work": {

      "order": 0.7

    }

  }

}
```

## PUT /\_matrix/client/v3/user/{userId}/rooms/{roomId}/tags/{tag}

---

Add a tag to the room.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The ID of the room to add a tag to. |
| `tag` | `string` | **Required:** The tag to add. |
| `userId` | `string` | **Required:** The id of the user to add a tag for. The access token must be authorized to make requests for this user ID. |

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `order` | `number` | A number in a range `[0,1]` describing a relative position of the room under the given tag. |

### Request body example

```json
{

  "order": 0.25

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The tag was successfully added. |

### 200 response

```json
{}
```

## DELETE /\_matrix/client/v3/user/{userId}/rooms/{roomId}/tags/{tag}

---

Remove a tag from the room.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The ID of the room to remove a tag from. |
| `tag` | `string` | **Required:** The tag to remove. |
| `userId` | `string` | **Required:** The id of the user to remove a tag for. The access token must be authorized to make requests for this user ID. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The tag was successfully removed. |

### 200 response

```json
{}
```

## Client Configuration

Clients can store custom config data for their account on their homeserver. This account data will be synced between different devices and can persist across installations on a particular device. Users may only view the account data for their own account.

The account data may be either global or scoped to a particular room. There is no inheritance mechanism here: a given `type` of data missing from a room's account data does not fall back to the global account data with the same `type`.

### Events

The client receives the account data as events in the `account_data` sections of a [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync) response.

These events can also be received in a [`/events`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3events) response or in the `account_data` section of a room in a `/sync` response. `m.tag` events appearing in `/events` will have a `room_id` with the room the tags are for.

### Client Behaviour

## GET /\_matrix/client/v3/user/{userId}/account\_data/{type}

---

Get some account data for the client. This config is only visible to the user that set the account data.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `type` | `string` | **Required:** The event type of the account data to get. Custom types should be namespaced to avoid clashes. |
| `userId` | `string` | **Required:** The ID of the user to get account data for. The access token must be authorized to make requests for this user ID. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The account data content for the given type. |
| `403` | The access token provided is not authorized to retrieve this user's account data. Errcode: `M_FORBIDDEN`. |
| `404` | No account data has been provided for this user with the given `type`. Errcode: `M_NOT_FOUND`. |

### 200 response

```json
{

  "custom_account_data_key": "custom_config_value"

}
```

### 403 response

```json
{

  "errcode": "M_FORBIDDEN",

  "error": "Cannot add account data for other users."

}
```

### 404 response

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "Account data not found."

}
```

## PUT /\_matrix/client/v3/user/{userId}/account\_data/{type}

---

Set some account data for the client. This config is only visible to the user that set the account data. The config will be available to clients through the top-level `account_data` field in the homeserver response to [/sync](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync).

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `type` | `string` | **Required:** The event type of the account data to set. Custom types should be namespaced to avoid clashes. |
| `userId` | `string` | **Required:** The ID of the user to set account data for. The access token must be authorized to make requests for this user ID. |

### Request body

### Request body example

```json
{

  "custom_account_data_key": "custom_config_value"

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The account data was successfully added. |
| `400` | The request body is not a JSON object. Errcode: `M_BAD_JSON` or `M_NOT_JSON`. |
| `403` | The access token provided is not authorized to modify this user's account data. Errcode: `M_FORBIDDEN`. |
| `405` | This `type` of account data is controlled by the server; it cannot be modified by clients. Errcode: `M_BAD_JSON`. |

### 200 response

```json
{}
```

### 400 response

```json
{

  "errcode": "M_NOT_JSON",

  "error": "Content must be a JSON object."

}
```

### 403 response

```json
{

  "errcode": "M_FORBIDDEN",

  "error": "Cannot add account data for other users."

}
```

### 405 response

```json
{

  "errcode": "M_BAD_JSON",

  "error": "Cannot set m.fully_read through this API."

}
```

## GET /\_matrix/client/v3/user/{userId}/rooms/{roomId}/account\_data/{type}

---

Get some account data for the client on a given room. This config is only visible to the user that set the account data.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The ID of the room to get account data for. |
| `type` | `string` | **Required:** The event type of the account data to get. Custom types should be namespaced to avoid clashes. |
| `userId` | `string` | **Required:** The ID of the user to get account data for. The access token must be authorized to make requests for this user ID. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The account data content for the given type. |
| `400` | The given `roomID` is not a valid room ID. Errcode: `M_INVALID_PARAM`. |
| `403` | The access token provided is not authorized to retrieve this user's account data. Errcode: `M_FORBIDDEN`. |
| `404` | No account data has been provided for this user and this room with the given `type`. Errcode: `M_NOT_FOUND`. |

### 200 response

```json
{

  "custom_account_data_key": "custom_config_value"

}
```

### 400 response

```json
{

  "errcode": "M_INVALID_PARAM",

  "error": "@notaroomid:example.org is not a valid room ID."

}
```

### 403 response

```json
{

  "errcode": "M_FORBIDDEN",

  "error": "Cannot add account data for other users."

}
```

### 404 response

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "Room account data not found."

}
```

## PUT /\_matrix/client/v3/user/{userId}/rooms/{roomId}/account\_data/{type}

---

Set some account data for the client on a given room. This config is only visible to the user that set the account data. The config will be delivered to clients in the per-room entries via [/sync](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync).

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The ID of the room to set account data on. |
| `type` | `string` | **Required:** The event type of the account data to set. Custom types should be namespaced to avoid clashes. |
| `userId` | `string` | **Required:** The ID of the user to set account data for. The access token must be authorized to make requests for this user ID. |

### Request body

### Request body example

```json
{

  "custom_account_data_key": "custom_account_data_value"

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The account data was successfully added. |
| `400` | The request body is not a JSON object (errcode `M_BAD_JSON` or `M_NOT_JSON`), or the given `roomID` is not a valid room ID (errcode `M_INVALID_PARAM`). |
| `403` | The access token provided is not authorized to modify this user's account data. Errcode: `M_FORBIDDEN`. |
| `405` | This `type` of account data is controlled by the server; it cannot be modified by clients. Errcode: `M_BAD_JSON`. |

### 200 response

```json
{}
```

### 400 response

```json
{

  "errcode": "M_NOT_JSON",

  "error": "Content must be a JSON object."

}
```

### 403 response

```json
{

  "errcode": "M_FORBIDDEN",

  "error": "Cannot add account data for other users."

}
```

### 405 response

```json
{

  "errcode": "M_BAD_JSON",

  "error": "Cannot set m.fully_read through this API."

}
```