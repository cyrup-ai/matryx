# Ephemeral Data Units (EDUs)

## Overview

EDUs, by comparison to PDUs, do not have an ID, a room ID, or a list of "previous" IDs. They are intended to be non-persistent data such as user presence, typing notifications, etc.

## Ephemeral Data Unit Format

An ephemeral data unit.

| Name | Type | Description |
| --- | --- | --- |
| `content` | `object` | **Required:** The content of the ephemeral message. |
| `edu_type` | `string` | **Required:** The type of ephemeral message. |

### Basic Example

```json
{
  "content": {
    "key": "value"
  },
  "edu_type": "m.presence"
}
```

## Typing Notifications

When a server's users send typing notifications, those notifications need to be sent to other servers in the room so their users are aware of the same state. Receiving servers should verify that the user is in the room, and is a user belonging to the sending server.

### m.typing

A typing notification EDU for a user in a room.

| Name | Type | Description |
| --- | --- | --- |
| `content` | `Typing Notification` | **Required:** The typing notification. |
| `edu_type` | `string` | **Required:** The string `m.typing` |

#### Typing Notification Content

| Name | Type | Description |
| --- | --- | --- |
| `room_id` | `string` | **Required:** The room where the user's typing status has been updated. |
| `typing` | `boolean` | **Required:** Whether the user is typing in the room or not. |
| `user_id` | `string` | **Required:** The user ID that has had their typing status changed. |

#### Example

```json
{
  "content": {
    "room_id": "!somewhere:matrix.org",
    "typing": true,
    "user_id": "@john:matrix.org"
  },
  "edu_type": "m.typing"
}
```

## Presence

The server API for presence is based entirely on exchange of the following EDUs. There are no PDUs or Federation Queries involved.

Servers should only send presence updates for users that the receiving server would be interested in. Such as the receiving server sharing a room with a given user.

### m.presence

An EDU representing presence updates for users of the sending homeserver.

| Name | Type | Description |
| --- | --- | --- |
| `content` | `Presence Update` | **Required:** The presence updates and requests. |
| `edu_type` | `string` | **Required:** The string `m.presence` |

#### Presence Update Content

| Name | Type | Description |
| --- | --- | --- |
| `push` | `[User Presence Update]` | **Required:** A list of presence updates that the receiving server is likely to be interested in. |

#### User Presence Update

| Name | Type | Description |
| --- | --- | --- |
| `currently_active` | `boolean` | True if the user is likely to be interacting with their client. This may be indicated by the user having a `last_active_ago` within the last few minutes. Defaults to false. |
| `last_active_ago` | `integer` | **Required:** The number of milliseconds that have elapsed since the user last did something. |
| `presence` | `string` | **Required:** The presence of the user. One of: `[offline, unavailable, online]`. |
| `status_msg` | `string` | An optional description to accompany the presence. |
| `user_id` | `string` | **Required:** The user ID this presence EDU is for. |

#### Example

```json
{
  "content": {
    "push": [
      {
        "currently_active": true,
        "last_active_ago": 5000,
        "presence": "online",
        "status_msg": "Making cupcakes",
        "user_id": "@john:matrix.org"
      }
    ]
  },
  "edu_type": "m.presence"
}
```

## Receipts

Receipts are EDUs used to communicate a marker for a given event. Currently the only kind of receipt supported is a "read receipt", or where in the event graph the user has read up to.

Read receipts for events that a user sent do not need to be sent. It is implied that by sending the event the user has read up to the event.

### m.receipt

An EDU representing receipt updates for users of the sending homeserver. When receiving receipts, the server should only update entries that are listed in the EDU. Receipts previously received that do not appear in the EDU should not be removed or otherwise manipulated.

| Name | Type | Description |
| --- | --- | --- |
| `content` | `{Room ID: Room Receipts}` | **Required:** Receipts for a particular room. The string key is the room ID for which the receipts under it belong. |
| `edu_type` | `string` | **Required:** The string `m.receipt` |

#### Room Receipts

| Name | Type | Description |
| --- | --- | --- |
| `m.read` | `{User ID: User Read Receipt}` | **Required:** Read receipts for users in the room. The string key is the user ID the receipt belongs to. |

#### User Read Receipt

| Name | Type | Description |
| --- | --- | --- |
| `data` | `Read Receipt Metadata` | **Required:** Metadata for the read receipt. |
| `event_ids` | `[string]` | **Required:** The extremity event IDs that the user has read up to. |

#### Read Receipt Metadata

| Name | Type | Description |
| --- | --- | --- |
| `ts` | `integer` | **Required:** The timestamp in milliseconds when the read receipt was sent. |

#### Example

```json
{
  "content": {
    "!some_room:example.org": {
      "m.read": {
        "@john:matrix.org": {
          "data": {
            "ts": 1533358089009
          },
          "event_ids": [
            "$read_this_event:matrix.org"
          ]
        }
      }
    }
  },
  "edu_type": "m.receipt"
}
```

## Device List Updates

Details of a user's devices must be efficiently published to other users and kept up-to-date. This is critical for reliable end-to-end encryption, in order for users to know which devices are participating in a room. It's also required for to-device messaging to work.

Matrix uses a custom pubsub system for synchronising information about the list of devices for a given user over federation. When a server wishes to determine a remote user's device list for the first time, it should populate a local cache from the result of a `/user/keys/query` API on the remote server. However, subsequent updates to the cache should be applied by consuming `m.device_list_update` EDUs.

### m.device_list_update

Each new `m.device_list_update` EDU describes an incremental change to one device for a given user which should replace any existing entry in the local server's cache of that device list. Servers must send `m.device_list_update` EDUs to all the servers who share a room with a given local user, and must be sent whenever that user's device list changes (i.e. for new or deleted devices, when that user joins a room which contains servers which are not already receiving updates for that user's device list, or changes in device information such as the device's human-readable name).

Servers send `m.device_list_update` EDUs in a sequence per origin user, each with a unique `stream_id`. They also include a pointer to the most recent previous EDU(s) that this update is relative to in the `prev_id` field. To simplify implementation for clustered servers which could send multiple EDUs at the same time, the `prev_id` field should include all `m.device_list_update` EDUs which have not been yet been referenced in a EDU. If EDUs are emitted in series by a server, there should only ever be one `prev_id` in the EDU.

This forms a simple directed acyclic graph of `m.device_list_update` EDUs, showing which EDUs a server needs to have received in order to apply an update to its local copy of the remote user's device list. If a server receives an EDU which refers to a `prev_id` it does not recognise, it must resynchronise its list by calling the `/user/keys/query` API and resume the process. The response contains a `stream_id` which should be used to correlate with subsequent `m.device_list_update` EDUs.

| Name | Type | Description |
| --- | --- | --- |
| `content` | `Device List Update` | **Required:** The device list update information. |
| `edu_type` | `string` | **Required:** The string `m.device_list_update` |

#### Device List Update Content

| Name | Type | Description |
| --- | --- | --- |
| `device_display_name` | `string` | The display name for the device. May be missing or empty. |
| `device_id` | `string` | **Required:** The device ID being updated. |
| `deleted` | `boolean` | True if the device is being deleted. Defaults to false. |
| `keys` | `object` | The device's public keys. May be missing if the device is being deleted. |
| `prev_id` | `[string]` | **Required:** The stream IDs of all previous `m.device_list_update` EDUs sent for this user that have not yet been referenced in a subsequent EDU's `prev_id` field. For the first EDU sent for a user, this should be an empty list. |
| `stream_id` | `integer` | **Required:** An ID to help correlate updates. This should be unique for each new EDU for the user. |
| `user_id` | `string` | **Required:** The user ID who owns the device. |

#### Example

```json
{
  "content": {
    "device_display_name": "Mobile Client",
    "device_id": "QBUAZIFURK",
    "keys": {
      "algorithms": ["m.olm.v1.curve25519-aes-sha2", "m.megolm.v1.aes-sha2"],
      "device_id": "QBUAZIFURK",
      "keys": {
        "curve25519:QBUAZIFURK": "3C5BFWi2Y8MaVvjM8M22DBmh24PmgR0nPvJOWpKnKdUp",
        "ed25519:QBUAZIFURK": "lEuiRJBit0IG6nUf5pUzWTUUQSJLl2x+2nYq2C9rXlMJFgADx4yC0v5rBFhPfK6H2Vp3v1lFqq8xK5MnvZG7+A"
      },
      "signatures": {
        "@alice:matrix.org": {
          "ed25519:QBUAZIFURK": "FLWxXqGbwrb8SM3Y4zg5PB4y+3mGfbjm5V4Ja26zLnGiRzh7kq7oqQq+vSUzIJzQE8qJzQb2bZAhNn6kFEDvAw"
        }
      },
      "user_id": "@alice:matrix.org"
    },
    "prev_id": ["2"],
    "stream_id": 3,
    "user_id": "@alice:matrix.org"
  },
  "edu_type": "m.device_list_update"
}
```

## EDU Processing Rules

### Transaction Integration

EDUs are sent within transactions alongside PDUs using the `PUT /_matrix/federation/v1/send/{txnId}` endpoint. The transaction format includes:

- `edus`: List of EDUs to process (maximum 100 EDUs per transaction)
- Each EDU must have valid `edu_type` and `content` fields
- EDUs are processed after PDU validation but before transaction response

### Delivery Guarantees

- EDUs are best-effort delivery (unlike PDUs which require acknowledgment)
- Servers should not retry failed EDU delivery
- Missing EDUs should not prevent room operation
- EDUs should be processed in the order received when possible

### Rate Limiting

- Servers should implement rate limiting for EDU processing
- Typing notifications should have short TTL and be deduplicated
- Presence updates should be batched and deduplicated
- Receipt processing should be batched by room

### Validation

- Verify sender server owns the user for user-specific EDUs
- Validate user membership in room for room-specific EDUs (typing, receipts)
- Ignore EDUs for unknown users or rooms
- Apply reasonable size limits to EDU content

### Storage

- EDUs are ephemeral and should not be persisted long-term
- Presence state should be cached with appropriate TTL
- Read receipts should update local receipt state
- Device list updates should update device caches
- Typing notifications should have short-lived local state