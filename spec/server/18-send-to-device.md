# Matrix Server-Server API: Send-to-Device Messaging

*Federation protocol specification for direct device-to-device messaging in the Matrix ecosystem.*

---

## Overview

Send-to-device messaging enables servers to push events directly to specific devices on remote servers without involving PDUs or Federation Queries. This is essential for maintaining end-to-end encrypted message channels between local and remote devices.

---

## Send-to-device messaging

The server API for send-to-device messaging is based on the `m.direct_to_device` EDU. There are no PDUs or Federation Queries involved.

Each send-to-device message should be sent to the destination server using the following EDU:

## m.direct\_to\_device

---

An EDU that lets servers push send events directly to a specific device on a remote server - for instance, to maintain an Olm E2E encrypted message channel between a local and remote device.

| Name | Type | Description |
| --- | --- | --- |
| `content` | `[To Device Message](https://spec.matrix.org/unstable/server-server-api/#definition-mdirect_to_device_to-device-message)` | **Required:** The description of the direct-to-device message. |
| `edu_type` | `string` | **Required:** The string `m.direct_to_device`.  One of: `[m.direct_to_device]`. |

| Name | Type | Description |
| --- | --- | --- |
| `message_id` | `string` | **Required:** Unique ID for the message, used for idempotence. Arbitrary utf8 string, of maximum length 32 codepoints. |
| `messages` | `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): {string: Device Message Contents}}` | **Required:** The contents of the messages to be sent. These are arranged in a map of user IDs to a map of device IDs to message bodies. The device ID may also be `*`, meaning all known devices for the user. |
| `sender` | `string` | **Required:** User ID of the sender. |
| `type` | `string` | **Required:** Event type for the message. |

## Examples

```json
{

  "content": {

    "message_id": "hiezohf6Hoo7kaev",

    "messages": {

      "@alice:example.org": {

        "IWHQUZUIAH": {

          "algorithm": "m.megolm.v1.aes-sha2",

          "room_id": "!Cuyf34gef24t:localhost",

          "session_id": "X3lUlvLELLYxeTx4yOVu6UDpasGEVO0Jbu+QFnm0cKQ",

          "session_key": "AgAAAADxKHa9uFxcXzwYoNueL5Xqi69IkD4sni8LlfJL7qNBEY..."

        }

      }

    },

    "sender": "@john:example.com",

    "type": "m.room_key_request"

  },

  "edu_type": "m.direct_to_device"

}
```