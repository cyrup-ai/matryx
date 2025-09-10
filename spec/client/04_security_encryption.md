# Matrix Client-Server API: Security and Encryption

This document contains the security and encryption features from the Matrix Client-Server specification, including send-to-device messaging, device management, and end-to-end encryption.

## Send-to-Device messaging

This module provides a means by which clients can exchange signalling messages without them being stored permanently as part of a shared communication history. A message is delivered exactly once to each client device.

The primary motivation for this API is exchanging data that is meaningless or undesirable to persist in the room DAG - for example, one-time authentication tokens or key data. It is not intended for conversational data, which should be sent using the normal [`/rooms/<room_id>/send`](https://spec.matrix.org/unstable/client-server-api/#put_matrixclientv3roomsroomidsendeventtypetxnid) API for consistency throughout Matrix.

### Client behaviour

To send a message to other devices, a client should call [`/sendToDevice`](https://spec.matrix.org/unstable/client-server-api/#put_matrixclientv3sendtodeviceeventtypetxnid). Only one message can be sent to each device per transaction, and they must all have the same event type. The device ID in the request body can be set to `*` to request that the message be sent to all known devices.

If there are send-to-device messages waiting for a client, they will be returned by [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync), as detailed in [Extensions to /sync](https://spec.matrix.org/unstable/client-server-api/#extensions-to-sync). Clients should inspect the `type` of each returned event, and ignore any they do not understand.

### Server behaviour

Servers should store pending messages for local users until they are successfully delivered to the destination device. When a client calls [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync) with an access token which corresponds to a device with pending messages, the server should list the pending messages, in order of arrival, in the response body.

When the client calls `/sync` again with the `next_batch` token from the first response, the server should infer that any send-to-device messages in that response have been delivered successfully, and delete them from the store.

If there is a large queue of send-to-device messages, the server should limit the number sent in each `/sync` response. 100 messages is recommended as a reasonable limit.

If the client sends messages to users on remote domains, those messages should be sent on to the remote servers via [federation](https://spec.matrix.org/unstable/server-server-api/#send-to-device-messaging).

### Protocol definitions

## PUT /_matrix/client/v3/sendToDevice/{eventType}/{txnId}

---

This endpoint is used to send send-to-device events to a set of client devices.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `eventType` | `string` | **Required:** The type of event to send. |
| `txnId` | `string` | **Required:** The [transaction ID](https://spec.matrix.org/unstable/client-server-api/#transaction-identifiers) for this event. Clients should generate an ID unique across requests with the same access token; it will be used by the server to ensure idempotency of requests. |

#### Request body

| Name | Type | Description |
| --- | --- | --- |
| `messages` | `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): {string: EventContent}}` | **Required:** The messages to send. A map from user ID, to a map from device ID to message body. The device ID may also be `*`, meaning all known devices for the user. |

#### Request body example

```json
{
  "messages": {
    "@alice:example.com": {
      "TLLBEANAAG": {
        "example_content_key": "value"
      }
    }
  }
}
```

---

### Responses

| Status | Description |
| --- | --- |
| `200` | The message was successfully sent. |

#### 200 response

```json
{}
```

#### Extensions to /sync

This module adds the following properties to the [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync) response:

| Parameter | Type | Description |
| --- | --- | --- |
| to\_device | ToDevice | Optional. Information on the send-to-device messages for the client device. |

`ToDevice`

| Parameter | Type | Description |
| --- | --- | --- |
| events | \[Event\] | List of send-to-device messages. |

`Event`

| Parameter | Type | Description |
| --- | --- | --- |
| content | EventContent | The content of this event. The fields in this object will vary depending on the type of event. |
| sender | string | The Matrix user ID of the user who sent this event. |
| type | string | The type of event. |

Example response:

```json
{
  "next_batch": "s72595_4483_1934",
  "rooms": {"leave": {}, "join": {}, "invite": {}},
  "to_device": {
    "events": [
      {
        "sender": "@alice:example.com",
        "type": "m.new_device",
        "content": {
          "device_id": "XYZABCDE",
          "rooms": ["!726s6s6q:example.com"]
        }
      }
    ]
  }
}
```

## Device Management

This module provides a means for a user to manage their [devices](https://spec.matrix.org/unstable/#devices).

### Client behaviour

Clients that implement this module should offer the user a list of registered devices, as well as the means to update their display names. Clients should also allow users to delete disused devices.

## POST /_matrix/client/v3/delete_devices

---

This API endpoint uses the [User-Interactive Authentication API](https://spec.matrix.org/unstable/client-server-api/#user-interactive-authentication-api).

Deletes the given devices, and invalidates any access token associated with them.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

### Request

#### Request body

| Name | Type | Description |
| --- | --- | --- |
| `auth` | `[Authentication Data](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3delete_devices_request_authentication-data)` | Additional authentication information for the user-interactive authentication API. |
| `devices` | `[string]` | **Required:** The list of device IDs to delete. |

| Name | Type | Description |
| --- | --- | --- |
| `session` | `string` | The value of the session key given by the homeserver. |
| `type` | `string` | The authentication type that the client is attempting to complete. May be omitted if `session` is given, and the client is reissuing a request which it believes has been completed out-of-band (for example, via the [fallback mechanism](https://spec.matrix.org/unstable/client-server-api/#fallback)). |
| <Other properties> |  | Keys dependent on the login type |

#### Request body example

---

### Responses

| Status | Description |
| --- | --- |
| `200` | The devices were successfully removed, or had been removed previously. |
| `401` | The homeserver requires additional authentication information. |

#### 200 response

```json
{}
```

#### 401 response

| Name | Type | Description |
| --- | --- | --- |
| `completed` | `[string]` | A list of the stages the client has completed successfully |
| `flows` | `[[Flow information](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3delete_devices_response-401_flow-information)]` | **Required:** A list of the login flows supported by the server for this API. |
| `params` | `{string: object}` | Contains any information that the client will need to know in order to use a given type of authentication. For each login type presented, that type may be present as a key in this dictionary. For example, the public part of an OAuth client ID could be given here. |
| `session` | `string` | This is a session identifier that the client must pass back to the home server, if one is provided, in subsequent attempts to authenticate in the same API call. |

| Name | Type | Description |
| --- | --- | --- |
| `stages` | `[string]` | **Required:** The login type of each of the stages required to complete this authentication flow |

```json
{
  "completed": [
    "example.type.foo"
  ],
  "flows": [
    {
      "stages": [
        "example.type.foo"
      ]
    }
  ],
  "params": {
    "example.type.baz": {
      "example_key": "foobar"
    }
  },
  "session": "xxxxxxyz"
}
```

## GET /_matrix/client/v3/devices

---

Gets information about all devices for the current user.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

### Request

No request parameters or request body.

---

### Responses

| Status | Description |
| --- | --- |
| `200` | Device information |

#### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `devices` | `[[Device](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3devices_response-200_device)]` | A list of all registered devices for this user. |

| Name | Type | Description |
| --- | --- | --- |
| `device_id` | `string` | **Required:** Identifier of this device. |
| `display_name` | `string` | Display name set by the user for this device. Absent if no name has been set. |
| `last_seen_ip` | `string` | The IP address where this device was last seen. (May be a few minutes out of date, for efficiency reasons). |
| `last_seen_ts` | `integer` | The timestamp (in milliseconds since the unix epoch) when this devices was last seen. (May be a few minutes out of date, for efficiency reasons). |

```json
{
  "devices": [
    {
      "device_id": "QBUAZIFURK",
      "display_name": "android",
      "last_seen_ip": "1.2.3.4",
      "last_seen_ts": 1474491775024
    }
  ]
}
```

## GET /_matrix/client/v3/devices/{deviceId}

---

Gets information on a single device, by device id.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `deviceId` | `string` | **Required:** The device to retrieve. |

---

### Responses

| Status | Description |
| --- | --- |
| `200` | Device information |
| `404` | The current user has no device with the given ID. |

#### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `device_id` | `string` | **Required:** Identifier of this device. |
| `display_name` | `string` | Display name set by the user for this device. Absent if no name has been set. |
| `last_seen_ip` | `string` | The IP address where this device was last seen. (May be a few minutes out of date, for efficiency reasons). |
| `last_seen_ts` | `integer` | The timestamp (in milliseconds since the unix epoch) when this devices was last seen. (May be a few minutes out of date, for efficiency reasons). |

```json
{
  "device_id": "QBUAZIFURK",
  "display_name": "android",
  "last_seen_ip": "1.2.3.4",
  "last_seen_ts": 1474491775024
}
```

## PUT /_matrix/client/v3/devices/{deviceId}

---

Updates the metadata on the given device.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `deviceId` | `string` | **Required:** The device to update. |

#### Request body

| Name | Type | Description |
| --- | --- | --- |
| `display_name` | `string` | The new display name for this device. If not given, the display name is unchanged. |

#### Request body example

```json
{
  "display_name": "My other phone"
}
```

---

### Responses

| Status | Description |
| --- | --- |
| `200` | The device was successfully updated. |
| `404` | The current user has no device with the given ID. |

#### 200 response

```json
{}
```

## DELETE /_matrix/client/v3/devices/{deviceId}

---

This API endpoint uses the [User-Interactive Authentication API](https://spec.matrix.org/unstable/client-server-api/#user-interactive-authentication-api).

Deletes the given device, and invalidates any access token associated with it.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `deviceId` | `string` | **Required:** The device to delete. |

#### Request body

| Name | Type | Description |
| --- | --- | --- |
| `auth` | `[Authentication Data](https://spec.matrix.org/unstable/client-server-api/#delete_matrixclientv3devicesdeviceid_request_authentication-data)` | Additional authentication information for the user-interactive authentication API. |

| Name | Type | Description |
| --- | --- | --- |
| `session` | `string` | The value of the session key given by the homeserver. |
| `type` | `string` | The authentication type that the client is attempting to complete. May be omitted if `session` is given, and the client is reissuing a request which it believes has been completed out-of-band (for example, via the [fallback mechanism](https://spec.matrix.org/unstable/client-server-api/#fallback)). |
| <Other properties> |  | Keys dependent on the login type |

#### Request body example

---

### Responses

| Status | Description |
| --- | --- |
| `200` | The device was successfully removed, or had been removed previously. |
| `401` | The homeserver requires additional authentication information. |

#### 200 response

```json
{}
```

#### 401 response

| Name | Type | Description |
| --- | --- | --- |
| `completed` | `[string]` | A list of the stages the client has completed successfully |
| `flows` | `[[Flow information](https://spec.matrix.org/unstable/client-server-api/#delete_matrixclientv3devicesdeviceid_response-401_flow-information)]` | **Required:** A list of the login flows supported by the server for this API. |
| `params` | `{string: object}` | Contains any information that the client will need to know in order to use a given type of authentication. For each login type presented, that type may be present as a key in this dictionary. For example, the public part of an OAuth client ID could be given here. |
| `session` | `string` | This is a session identifier that the client must pass back to the home server, if one is provided, in subsequent attempts to authenticate in the same API call. |

| Name | Type | Description |
| --- | --- | --- |
| `stages` | `[string]` | **Required:** The login type of each of the stages required to complete this authentication flow |

```json
{
  "completed": [
    "example.type.foo"
  ],
  "flows": [
    {
      "stages": [
        "example.type.foo"
      ]
    }
  ],
  "params": {    "example.type.baz": {
      "example_key": "foobar"
    }
  },
  "session": "xxxxxxyz"
}
```

### Security considerations

Deleting devices has security implications: it invalidates the access\_token assigned to the device, so an attacker could use it to log out the real user (and do it repeatedly every time the real user tries to log in to block the attacker). Servers should require additional authentication beyond the access token when deleting devices (for example, requiring that the user resubmit their password).

The display names of devices are publicly visible. Clients should consider advising the user of this.

## End-to-End Encryption

Matrix optionally supports end-to-end encryption, allowing rooms to be created whose conversation contents are not decryptable or interceptable on any of the participating homeservers.

### Key Distribution

Encryption and Authentication in Matrix is based around public-key cryptography. The Matrix protocol provides a basic mechanism for exchange of public keys, though an out-of-band channel is required to exchange fingerprints between users to build a web of trust.

#### Overview

1. Bob publishes the public keys and supported algorithms for his device. This may include long-term identity keys, and/or one-time keys.
```
+----------+  +--------------+
      | Bob's HS |  | Bob's Device |
      +----------+  +--------------+
            |              |
            |<=============|
              /keys/upload
```
2. Alice requests Bob's public identity keys and supported algorithms.
```
+----------------+  +------------+  +----------+
      | Alice's Device |  | Alice's HS |  | Bob's HS |
      +----------------+  +------------+  +----------+
             |                  |               |
             |=================>|==============>|
               /keys/query        <federation>
```
3. Alice selects an algorithm and claims any one-time keys needed.
```
+----------------+  +------------+  +----------+
      | Alice's Device |  | Alice's HS |  | Bob's HS |
      +----------------+  +------------+  +----------+
             |                  |               |
             |=================>|==============>|
               /keys/claim         <federation>
```

#### Key algorithms

Different key algorithms are used for different purposes. Each key algorithm is identified by a name and is represented in a specific way.

The name `ed25519` corresponds to the [Ed25519](http://ed25519.cr.yp.to/) signature algorithm. The key is a 32-byte Ed25519 public key, encoded using [unpadded Base64](https://spec.matrix.org/unstable/appendices/#unpadded-base64). Example:

```
"SogYyrkTldLz0BXP+GYWs0qaYacUI0RleEqNT8J3riQ"
```

The name `curve25519` corresponds to the [Curve25519](https://cr.yp.to/ecdh.html) ECDH algorithm. The key is a 32-byte Curve25519 public key, encoded using [unpadded Base64](https://spec.matrix.org/unstable/appendices/#unpadded-base64). Example:

```
"JGLn/yafz74HB2AbPLYJWIVGnKAtqECOBf11yyXac2Y"
```

The name `signed_curve25519` also corresponds to the Curve25519 ECDH algorithm, but the key is signed so that it can be authenticated. A key using this algorithm is represented by an object with the following properties:

`KeyObject`

| Parameter | Type | Description |
| --- | --- | --- |
| key | string | **Required.** The unpadded Base64-encoded 32-byte Curve25519 public key. |
| signatures | Signatures | **Required.** Signatures of the key object. The signature is calculated using the process described at [Signing JSON](https://spec.matrix.org/unstable/appendices/#signing-json). |
| fallback | boolean | Indicates whether this is a [fallback key](https://spec.matrix.org/unstable/client-server-api/#one-time-and-fallback-keys). Defaults to `false`. |

Example:

```json
{
  "key":"06UzBknVHFMwgi7AVloY7ylC+xhOhEX4PkNge14Grl8",
  "signatures": {
    "@user:example.com": {
      "ed25519:EGURVBUNJP": "YbJva03ihSj5mPk+CHMJKUKlCXCPFXjXOK6VqBnN9nA2evksQcTGn6hwQfrgRHIDDXO2le49x7jnWJHMJrJoBQ"
    }
  }
}
```

`ed25519` and `curve25519` keys are used for [device keys](https://spec.matrix.org/unstable/client-server-api/#device-keys). Additionally, `ed25519` keys are used for [cross-signing keys](https://spec.matrix.org/unstable/client-server-api/#cross-signing).

`signed_curve25519` keys are used for [one-time and fallback keys](https://spec.matrix.org/unstable/client-server-api/#one-time-and-fallback-keys).

#### Device keys

Each device should have one Ed25519 signing key. This key should be generated on the device from a cryptographically secure source, and the private part of the key should never be exported from the device. This key is used as the fingerprint for a device by other clients, and signs the device's other keys.

A device will generally need to generate a number of additional keys. Details of these will vary depending on the messaging algorithm in use.

For Olm version 1, each device also requires a single Curve25519 identity key.

#### One-time and fallback keys

In addition to the device keys, which are long-lived, some encryption algorithms require that devices may also have a number of one-time keys, which are only used once and discarded after use. For Olm version 1, devices use `signed_curve25519` one-time keys, signed by the device's Ed25519 key.

Devices will generate one-time keys and upload them to the server; these will later be [claimed](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysclaim) by other users. Servers must ensure that each one-time key is only claimed once: a homeserver should discard the one time key once it has been given to another user.

**\[Added in `v1.2`\]** Fallback keys are similar to one-time keys, but are not consumed once used. If a fallback key has been uploaded, it will be returned by the server when the device has run out of one-time keys and a user tries to claim a key. Fallback keys should be replaced with new fallback keys as soon as possible after they have been used.

Devices will be informed, [via `/sync`](https://spec.matrix.org/unstable/client-server-api/#e2e-extensions-to-sync), about the number of one-time keys remaining that can be claimed, as well as whether the fallback keys have been used. The device can thus ensure that, while it is online, there is a sufficient supply of one-time keys available, and that the fallback keys get replaced if they have been used.

A device uploads the public parts of identity keys to their homeserver as a signed JSON object, using the [`/keys/upload`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysupload) API. The JSON object must include the public part of the device's Ed25519 key, and must be signed by that key, as described in [Signing JSON](https://spec.matrix.org/unstable/appendices/#signing-json).

One-time and fallback keys are also uploaded to the homeserver using the [`/keys/upload`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysupload) API. New one-time and fallback keys are uploaded as needed. Fallback keys for key algorithms whose format is a signed JSON object should contain a property named `fallback` with a value of `true`.

Devices must store the private part of each key they upload. They can discard the private part of a one-time key when they receive a message using that key. However it's possible that a one-time key given out by a homeserver will never be used, so the device that generates the key will never know that it can discard the key. Therefore a device could end up trying to store too many private keys. A device that is trying to store too many private keys may discard keys starting with the oldest.

#### Tracking the device list for a user

Before Alice can send an encrypted message to Bob, she needs a list of each of his devices and the associated identity keys, so that she can establish an encryption session with each device. This list can be obtained by calling [`/keys/query`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysquery), passing Bob's user ID in the `device_keys` parameter.

From time to time, Bob may add new devices, and Alice will need to know this so that she can include his new devices for later encrypted messages. A naive solution to this would be to call [`/keys/query`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysquery) before sending each message -however, the number of users and devices may be large and this would be inefficient.

It is therefore expected that each client will maintain a list of devices for a number of users (in practice, typically each user with whom we share an encrypted room). Furthermore, it is likely that this list will need to be persisted between invocations of the client application (to preserve device verification data and to alert Alice if Bob suddenly gets a new device).

Alice's client can maintain a list of Bob's devices via the following process:

1. It first sets a flag to record that it is now tracking Bob's device list, and a separate flag to indicate that its list of Bob's devices is outdated. Both flags should be in storage which persists over client restarts.
2. It then makes a request to [`/keys/query`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysquery), passing Bob's user ID in the `device_keys` parameter. When the request completes, it stores the resulting list of devices in persistent storage, and clears the 'outdated' flag.
3. During its normal processing of responses to [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync), Alice's client inspects the `changed` property of the [`device_lists`](https://spec.matrix.org/unstable/client-server-api/#e2e-extensions-to-sync) field. If it is tracking the device lists of any of the listed users, then it marks the device lists for those users outdated, and initiates another request to [`/keys/query`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysquery) for them.
4. Periodically, Alice's client stores the `next_batch` field of the result from [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync) in persistent storage. If Alice later restarts her client, it can obtain a list of the users who have updated their device list while it was offline by calling [`/keys/changes`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3keyschanges), passing the recorded `next_batch` field as the `from` parameter. If the client is tracking the device list of any of the users listed in the response, it marks them as outdated. It combines this list with those already flagged as outdated, and initiates a [`/keys/query`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysquery) request for all of them.

#### Sending encrypted attachments

When encryption is enabled in a room, files should be uploaded encrypted on the homeserver.

In order to achieve this, a client should generate a single-use 256-bit AES key, and encrypt the file using AES-CTR. The counter should be 64-bit long, starting at 0 and prefixed by a random 64-bit Initialization Vector (IV), which together form a 128-bit unique counter block.

Then, the encrypted file can be uploaded to the homeserver. The key and the IV must be included in the room event along with the resulting `mxc://` in order to allow recipients to decrypt the file. As the event containing those will be Megolm encrypted, the server will never have access to the decrypted file.

A hash of the ciphertext must also be included, in order to prevent the homeserver from changing the file content.

A client should send the data as an encrypted `m.room.message` event, using either `m.file` as the msgtype, or the appropriate msgtype for the file type. The key is sent using the [JSON Web Key](https://tools.ietf.org/html/rfc7517#appendix-A.3) format, with a [W3C extension](https://w3c.github.io/webcrypto/#iana-section-jwk).

##### Extensions to m.room.message msgtypes

This module adds `file` and `thumbnail_file` properties, of type `EncryptedFile`, to `m.room.message` msgtypes that reference files, such as [m.file](https://spec.matrix.org/unstable/client-server-api/#mfile) and [m.image](https://spec.matrix.org/unstable/client-server-api/#mimage), replacing the `url` and `thumbnail_url` properties.

`EncryptedFile`

| Parameter | Type | Description |
| --- | --- | --- |
| url | string | **Required.** The URL to the file. |
| key | JWK | **Required.** A [JSON Web Key](https://tools.ietf.org/html/rfc7517#appendix-A.3) object. |
| iv | string | **Required.** The 128-bit unique counter block used by AES-CTR, encoded as unpadded base64. |
| hashes | {string: string} | **Required.** A map from an algorithm name to a hash of the ciphertext, encoded as unpadded base64. Clients should support the SHA-256 hash, which uses the key `sha256`. |
| v | string | **Required.** Version of the encrypted attachment's protocol. Must be `v2`. |

`JWK`

| Parameter | Type | Description |
| --- | --- | --- |
| kty | string | **Required.** Key type. Must be `oct`. |
| key\_ops | \[string\] | **Required.** Key operations. Must at least contain `encrypt` and `decrypt`. |
| alg | string | **Required.** Algorithm. Must be `A256CTR`. |
| k | string | **Required.** The key, encoded as urlsafe unpadded base64. |
| ext | boolean | **Required.** Extractable. Must be `true`. This is a [W3C extension](https://w3c.github.io/webcrypto/#iana-section-jwk). |

Example:

```json
{
  "content": {
    "body": "something-important.jpg",
    "file": {
      "url": "mxc://example.org/FHyPlCeYUSFFxlgbQYZmoEoe",
      "v": "v2",
      "key": {
        "alg": "A256CTR",
        "ext": true,
        "k": "aWF6-32KGYaC3A_FEUCk1Bt0JA37zP0wrStgmdCaW-0",
        "key_ops": ["encrypt","decrypt"],
        "kty": "oct"
      },
      "iv": "w+sE15fzSc0AAAAAAAAAAA",
      "hashes": {
        "sha256": "fdSLu/YkRx3Wyh3KQabP3rd6+SFiKg5lsJZQHtkSAYA"
      }
    },
    "info": {
      "mimetype": "image/jpeg",
      "h": 1536,
      "size": 422018,
      "thumbnail_file": {
        "hashes": {
          "sha256": "/NogKqW5bz/m8xHgFiH5haFGjCNVmUIPLzfvOhHdrxY"
        },
        "iv": "U+k7PfwLr6UAAAAAAAAAAA",
        "key": {
          "alg": "A256CTR",
          "ext": true,
          "k": "RMyd6zhlbifsACM1DXkCbioZ2u0SywGljTH8JmGcylg",
          "key_ops": ["encrypt", "decrypt"],
          "kty": "oct"
        },
        "url": "mxc://example.org/pmVJxyxGlmxHposwVSlOaEOv",
        "v": "v2"
      },
      "thumbnail_info": {
        "h": 768,
        "mimetype": "image/jpeg",
        "size": 211009,
        "w": 432
      },
      "w": 864
    },
    "msgtype": "m.image"
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@example:example.org",
  "type": "m.room.message",
  "unsigned": {
      "age": 1234
  }
}
```

### Device verification

Before Alice sends Bob encrypted data, or trusts data received from him, she may want to verify that she is actually communicating with him, rather than a man-in-the-middle. This verification process requires an out-of-band channel: there is no way to do it within Matrix without trusting the administrators of the homeservers.

In Matrix, verification works by Alice meeting Bob in person, or contacting him via some other trusted medium, and using one of the verification methods defined below to interactively verify Bob's devices. Alice and Bob may also read aloud their unpadded base64 encoded Ed25519 public key, as returned by `/keys/query`.

Device verification may reach one of several conclusions. For example:

- Alice may "accept" the device. This means that she is satisfied that the device belongs to Bob. She can then encrypt sensitive material for that device, and knows that messages received were sent from that device.
- Alice may "reject" the device. She will do this if she knows or suspects that Bob does not control that device (or equivalently, does not trust Bob). She will not send sensitive material to that device, and cannot trust messages apparently received from it.
- Alice may choose to skip the device verification process. She is not able to verify that the device actually belongs to Bob, but has no reason to suspect otherwise. The encryption protocol continues to protect against passive eavesdroppers.

#### Key verification framework

Verifying keys manually by reading out the Ed25519 key is not very user-friendly, and can lead to errors. In order to help mitigate errors, and to make the process easier for users, some verification methods are supported by the specification and use messages exchanged by the user's devices to assist in the verification. The methods all use a common framework for negotiating the key verification.

Verification messages can be sent either in a room shared by the two parties, which should be a [direct messaging](https://spec.matrix.org/unstable/client-server-api/#direct-messaging) room between the two parties, or by using [to-device](https://spec.matrix.org/unstable/client-server-api/#send-to-device-messaging) messages sent directly between the two devices involved. In both cases, the messages exchanged are similar, with minor differences as detailed below. Verifying between two different users should be performed using in-room messages, whereas verifying two devices belonging to the same user should be performed using to-device messages.

A key verification session is identified by an ID that is established by the first message sent in that session. For verifications using in-room messages, the ID is the event ID of the initial message, and for verifications using to-device messages, the first message contains a `transaction_id` field that is shared by the other messages of that session.

In general, verification operates as follows:

- Alice requests a key verification with Bob by sending a key verification request event. If the verification is being requested in a room, this will be an event with type [`m.room.message` and `msgtype: m.key.verification.request`](https://spec.matrix.org/unstable/client-server-api/#mroommessagemkeyverificationrequest); if the verification is being requested using to-device messaging, this will be an event with type [`m.key.verification.request`](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationrequest). This event indicates the verification methods that Alice's client supports. (Note that "Alice" and "Bob" may in fact be the same user, in the case where a user is verifying their own devices.)
- Bob's client prompts Bob to accept the key verification. When Bob accepts the verification, Bob's client sends an [`m.key.verification.ready`](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationready) event. This event indicates the verification methods, corresponding to the verification methods supported by Alice's client, that Bob's client supports.
- Alice's or Bob's devices allow their users to select one of the verification methods supported by both devices to use for verification. When Alice or Bob selects a verification method, their device begins the verification by sending an [`m.key.verification.start`](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationstart) event, indicating the selected verification method. Note that if there is only one verification method in common between the devices then the receiver's device (Bob) can auto-select it.
- Alice and Bob complete the verification as defined by the selected verification method. This could involve their clients exchanging messages, Alice and Bob exchanging information out-of-band, and/or Alice and Bob interacting with their devices.
- Alice's and Bob's clients send [`m.key.verification.done`](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationdone) events to indicate that the verification was successful.

Verifications can be cancelled by either device at any time by sending an [`m.key.verification.cancel`](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationcancel) event with a `code` field that indicates the reason it was cancelled.

When using to-device messages, Alice may not know which of Bob's devices to verify, or may not want to choose a specific device. In this case, Alice will send `m.key.verification.request` events to all of Bob's devices. All of these events will use the same transaction ID. When Bob accepts or declines the verification on one of his devices (sending either an `m.key.verification.ready` or `m.key.verification.cancel` event), Alice will send an `m.key.verification.cancel` event to Bob's other devices with a `code` of `m.accepted` in the case where Bob accepted the verification, or `m.user` in the case where Bob rejected the verification. This yields the following handshake when using to-device messages, assuming both Alice and Bob each have 2 devices, Bob's first device accepts the key verification request, and Alice's second device initiates the request. Note how Alice's first device is not involved in the request or verification process. Also note that, although in this example, Bob's device sends the `m.key.verification.start`, Alice's device could also send that message. As well, the order of the `m.key.verification.done` messages could be reversed.

```
+---------------+ +---------------+                    +-------------+ +-------------+
    | AliceDevice1  | | AliceDevice2  |                    | BobDevice1  | | BobDevice2  |
    +---------------+ +---------------+                    +-------------+ +-------------+
            |                 |                                   |               |
            |                 | m.key.verification.request        |               |
            |                 |---------------------------------->|               |
            |                 |                                   |               |
            |                 | m.key.verification.request        |               |
            |                 |-------------------------------------------------->|
            |                 |                                   |               |
            |                 |          m.key.verification.ready |               |
            |                 |<----------------------------------|               |
            |                 |                                   |               |
            |                 | m.key.verification.cancel         |               |
            |                 |-------------------------------------------------->|
            |                 |                                   |               |
            |                 |          m.key.verification.start |               |
            |                 |<----------------------------------|               |
            |                 |                                   |               |
            .
            .                       (verification messages)
            .
            |                 |                                   |               |
            |                 |           m.key.verification.done |               |
            |                 |<----------------------------------|               |
            |                 |                                   |               |
            |                 | m.key.verification.done           |               |
            |                 |---------------------------------->|               |
            |                 |                                   |               |
```

In contrast with the case of using to-devices messages, when using in-room messages, Alice only sends one request event (an event with type `m.room.message` with `msgtype: m.key.verification.request`, rather than an event with type `m.key.verification.request`), to the room. In addition, Alice does not send an `m.key.verification.cancel` event to tell Bob's other devices that the request has already been accepted; instead, when Bob's other devices see his `m.key.verification.ready` event, they will know that the request has already been accepted, and that they should ignore the request.

When using in-room messages and the room has encryption enabled, clients should ensure that encryption does not hinder the verification. For example, if the verification messages are encrypted, clients must ensure that all the recipient's unverified devices receive the keys necessary to decrypt the messages, even if they would normally not be given the keys to decrypt messages in the room. Alternatively, verification messages may be sent unencrypted, though this is not encouraged.

Upon receipt of Alice's `m.key.verification.request` message, if Bob's device does not understand any of the methods, it should not cancel the request as one of his other devices may support the request. Instead, Bob's device should tell Bob that no supported method was found, and allow him to manually reject the request.

The prompt for Bob to accept/reject Alice's request (or the unsupported method prompt) should be automatically dismissed 10 minutes after the `timestamp` (in the case of to-device messages) or `origin_ts` (in the case of in-room messages) field or 2 minutes after Bob's client receives the message, whichever comes first, if Bob does not interact with the prompt. The prompt should additionally be hidden if an appropriate `m.key.verification.cancel` message is received.

If Bob rejects the request, Bob's client must send an `m.key.verification.cancel` event with `code` set to `m.user`. Upon receipt, Alice's device should tell her that Bob does not want to verify her device and, if the request was sent as a to-device message, send `m.key.verification.cancel` messages to all of Bob's devices to notify them that the request was rejected.

If Alice's and Bob's clients both send an `m.key.verification.start` message, and both specify the same verification method, then the `m.key.verification.start` message sent by the user whose ID is the lexicographically largest user ID should be ignored, and the situation should be treated the same as if only the user with the lexicographically smallest user ID had sent the `m.key.verification.start` message. In the case where the user IDs are the same (that is, when a user is verifying their own device), then the device IDs should be compared instead. If the two `m.key.verification.start` messages do not specify the same verification method, then the verification should be cancelled with a `code` of `m.unexpected_message`.

When verifying using to-device messages, an `m.key.verification.start` message can also be sent independently of any request, specifying the verification method to use. This behaviour is deprecated, and new clients should not begin verifications in this way. However, clients should handle such verifications started by other clients.

Individual verification methods may add additional steps, events, and properties to the verification messages. Event types for methods defined in this specification must be under the `m.key.verification` namespace and any other event types must be namespaced according to the Java package naming convention.

## m.room.message with msgtype: m.key.verification.request

---

Requests a key verification in a room. When requesting a key verification using to-device messaging, an event with type [`m.key.verification.request`](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationrequest) should be used.

| Event type: | Message event |
| --- | --- |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `body` | `string` | A fallback message to alert users that their client does not support the key verification framework, and that they should use a different method to verify keys. For example, "Alice is requesting to verify keys with you. However, your client does not support this method, so you will need to use the legacy method of key verification."  Clients that do support the key verification framework should hide the body and instead present the user with an interface to accept or reject the key verification. |
| `format` | `string` | The format used in the `formatted_body`. This is required if `formatted_body` is specified. Currently only `org.matrix.custom.html` is supported. |
| `formatted_body` | `string` | The formatted version of the `body`. This is required if `format` is specified. As with the `body`, clients that do support the key verification framework should hide the formatted body and instead present the user with an interface to accept or reject the key verification. |
| `from_device` | `string` | **Required:** The device ID which is initiating the request. |
| `methods` | `[string]` | **Required:** The verification methods supported by the sender. |
| `msgtype` | `string` | **Required:**  One of: `[m.key.verification.request]`. |
| `to` | `string` | **Required:** The user that the verification request is intended for. Users who are not named in this field and who did not send this event should ignore all other events that have an `m.reference` relationship with this event. |

### Examples

```json
{
  "content": {
    "body": "Alice is requesting to verify your device, but your client does not support verification, so you may need to use a different verification method.",
    "from_device": "AliceDevice2",
    "methods": [
      "m.sas.v1"
    ],
    "msgtype": "m.key.verification.request",
    "to": "@bob:example.org"
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@alice:example.org",
  "type": "m.room.message",
  "unsigned": {
    "age": 1234
  }
}
```

## m.key.verification.request

---

Requests a key verification using to-device messaging. When requesting a key verification in a room, a `m.room.message` should be used, with [`m.key.verification.request`](https://spec.matrix.org/unstable/client-server-api/#mroommessagemkeyverificationrequest) as msgtype.

| Event type: | Message event |
| --- | --- |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `from_device` | `string` | **Required:** The device ID which is initiating the request. |
| `methods` | `[string]` | **Required:** The verification methods supported by the sender. |
| `timestamp` | `integer` | Required when sent as a to-device message. The POSIX timestamp in milliseconds for when the request was made. If the request is in the future by more than 5 minutes or more than 10 minutes in the past, the message should be ignored by the receiver. |
| `transaction_id` | `string` | Required when sent as a to-device message. An opaque identifier for the verification request. Must be unique with respect to the devices involved. |

### Examples

```json
{
  "content": {
    "from_device": "AliceDevice2",
    "methods": [
      "m.sas.v1"
    ],
    "timestamp": 1559598944869,
    "transaction_id": "S0meUniqueAndOpaqueString"
  },
  "type": "m.key.verification.request"
}
```

## m.key.verification.ready

---

Accepts a key verification request. Sent in response to an `m.key.verification.request` event.

| Event type: | Message event |
| --- | --- |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `from_device` | `string` | **Required:** The device ID which is accepting the request. || `m.relates_to` | `[VerificationRelatesTo](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationready_verificationrelatesto)` | Required when sent as an in-room message. Indicates the `m.key.verification.request` that this message is related to. Note that for encrypted messages, this property should be in the unencrypted portion of the event. |
| `methods` | `[string]` | **Required:** The verification methods supported by the sender, corresponding to the verification methods indicated in the `m.key.verification.request` message. |
| `transaction_id` | `string` | Required when sent as a to-device message. The transaction ID of the verification request, as given in the `m.key.verification.request` message. |

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | The event ID of the `m.key.verification.request` that this message is related to. |
| `rel_type` | `string` | The relationship type. Currently, this can only be an [`m.reference`](https://spec.matrix.org/unstable/client-server-api/#reference-relations) relationship type.  One of: `[m.reference]`. |

### Examples

```json
{
  "content": {
    "from_device": "BobDevice1",
    "methods": [
      "m.sas.v1"
    ],
    "transaction_id": "S0meUniqueAndOpaqueString"
  },
  "type": "m.key.verification.ready"
}
```

## m.key.verification.start

---

Begins a key verification process. Typically sent as a [to-device](https://spec.matrix.org/unstable/client-server-api/#send-to-device-messaging) event. The `method` field determines the type of verification. The fields in the event will differ depending on the `method`. This definition includes fields that are in common among all variants.

| Event type: | Message event |
| --- | --- |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `from_device` | `string` | **Required:** The device ID which is initiating the process. |
| `m.relates_to` | `[VerificationRelatesTo](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationstart_verificationrelatesto)` | Required when sent as an in-room message. Indicates the `m.key.verification.request` that this message is related to. Note that for encrypted messages, this property should be in the unencrypted portion of the event. |
| `method` | `string` | **Required:** The verification method to use. |
| `next_method` | `string` | Optional method to use to verify the other user's key with. Applicable when the `method` chosen only verifies one user's key. This field will never be present if the `method` verifies keys both ways. |
| `transaction_id` | `string` | Required when sent as a to-device message. An opaque identifier for the verification process. Must be unique with respect to the devices involved. Must be the same as the `transaction_id` given in the `m.key.verification.request` if this process is originating from a request. |

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | The event ID of the `m.key.verification.request` that this message is related to. |
| `rel_type` | `string` | The relationship type. Currently, this can only be an [`m.reference`](https://spec.matrix.org/unstable/client-server-api/#reference-relations) relationship type.  One of: `[m.reference]`. |

### Examples

```json
{
  "content": {
    "from_device": "BobDevice1",
    "method": "m.sas.v1",
    "transaction_id": "S0meUniqueAndOpaqueString"
  },
  "type": "m.key.verification.start"
}
```
```json
{
  "content": {
    "from_device": "BobDevice1",
    "hashes": [
      "sha256"
    ],
    "key_agreement_protocols": [
      "curve25519"
    ],
    "message_authentication_codes": [
      "hkdf-hmac-sha256.v2",
      "hkdf-hmac-sha256"
    ],
    "method": "m.sas.v1",
    "short_authentication_string": [
      "decimal",
      "emoji"
    ],
    "transaction_id": "S0meUniqueAndOpaqueString"
  },
  "type": "m.key.verification.start"
}
```

## m.key.verification.done

---

Indicates that a verification process/request has completed successfully.

| Event type: | Message event |
| --- | --- |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `m.relates_to` | `[VerificationRelatesTo](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationdone_verificationrelatesto)` | Required when sent as an in-room message. Indicates the `m.key.verification.request` that this message is related to. Note that for encrypted messages, this property should be in the unencrypted portion of the event. |
| `transaction_id` | `string` | Required when sent as a to-device message. The opaque identifier for the verification process/request. |

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | The event ID of the `m.key.verification.request` that this message is related to. |
| `rel_type` | `string` | The relationship type. Currently, this can only be an [`m.reference`](https://spec.matrix.org/unstable/client-server-api/#reference-relations) relationship type.  One of: `[m.reference]`. |

### Examples

```json
{
  "content": {
    "transaction_id": "S0meUniqueAndOpaqueString"
  },
  "type": "m.key.verification.done"
}
```

## m.key.verification.cancel

---

Cancels a key verification process/request.

| Event type: | Message event |
| --- | --- |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `code` | `string` | **Required:** The error code for why the process/request was cancelled by the user. Error codes should use the Java package naming convention if not in the following list:  - `m.user`: The user cancelled the verification. - `m.timeout`: The verification process timed out. Verification processes can define their own timeout parameters. - `m.unknown_transaction`: The device does not know about the given transaction ID. - `m.unknown_method`: The device does not know how to handle the requested method. This should be sent for `m.key.verification.start` messages and messages defined by individual verification processes. - `m.unexpected_message`: The device received an unexpected message. Typically raised when one of the parties is handling the verification out of order. - `m.key_mismatch`: The key was not verified. - `m.user_mismatch`: The expected user did not match the user verified. - `m.invalid_message`: The message received was invalid. - `m.accepted`: A `m.key.verification.request` was accepted by a different device. The device receiving this error can ignore the verification request.  Clients should be careful to avoid error loops. For example, if a device sends an incorrect message and the client returns `m.invalid_message` to which it gets an unexpected response with `m.unexpected_message`, the client should not respond again with `m.unexpected_message` to avoid the other device potentially sending another error response. |
| `m.relates_to` | `[VerificationRelatesTo](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationcancel_verificationrelatesto)` | Required when sent as an in-room message. Indicates the `m.key.verification.request` that this message is related to. Note that for encrypted messages, this property should be in the unencrypted portion of the event. |
| `reason` | `string` | **Required:** A human readable description of the `code`. The client should only rely on this string if it does not understand the `code`. |
| `transaction_id` | `string` | Required when sent as a to-device message. The opaque identifier for the verification process/request. |

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | The event ID of the `m.key.verification.request` that this message is related to. |
| `rel_type` | `string` | The relationship type. Currently, this can only be an [`m.reference`](https://spec.matrix.org/unstable/client-server-api/#reference-relations) relationship type.  One of: `[m.reference]`. |

### Examples

```json
{
  "content": {
    "code": "m.user",
    "reason": "User rejected the key verification request",
    "transaction_id": "S0meUniqueAndOpaqueString"
  },
  "type": "m.key.verification.cancel"
}
```

#### Short Authentication String (SAS) verification

SAS verification is a user-friendly key verification process built off the common framework outlined above. SAS verification is intended to be a highly interactive process for users, and as such exposes verification methods which are easier for users to use.

The verification process is heavily inspired by Phil Zimmermann's ZRTP key agreement handshake. A key part of key agreement in ZRTP is the hash commitment: the party that begins the Diffie-Hellman key sharing sends a hash of their part of the Diffie-Hellman exchange, and does not send their part of the Diffie-Hellman exchange until they have received the other party's part. Thus an attacker essentially only has one attempt to attack the Diffie-Hellman exchange, and hence we can verify fewer bits while still achieving a high degree of security: if we verify n bits, then an attacker has a 1 in 2 <sup>n</sup> chance of success. For example, if we verify 40 bits, then an attacker has a 1 in 1,099,511,627,776 chance (or less than 1 in 10 <sup>12</sup> chance) of success. A failed attack would result in a mismatched Short Authentication String, alerting users to the attack.

To advertise support for this method, clients use the name `m.sas.v1` in the `methods` fields of the `m.key.verification.request` and `m.key.verification.ready` events.

The verification process takes place in two phases:

1. Key agreement phase (based on [ZRTP key agreement](https://tools.ietf.org/html/rfc6189#section-4.4.1)).
2. Key verification phase (based on HMAC).

The process between Alice and Bob verifying each other would be:

1. Alice and Bob establish a secure out-of-band connection, such as meeting in-person or a video call. "Secure" here means that either party cannot be impersonated, not explicit secrecy.
2. Alice and Bob begin a key verification using the key verification framework as described above.
3. Alice's device sends Bob's device an `m.key.verification.start` message. Alice's device ensures it has a copy of Bob's device key.
4. Bob's device receives the message and selects a key agreement protocol, hash algorithm, message authentication code, and SAS method supported by Alice's device.
5. Bob's device ensures it has a copy of Alice's device key.
6. Bob's device creates an ephemeral Curve25519 key pair (*K <sub>B</sub> <sup>private</sup>*, *K <sub>B</sub> <sup>public</sup>*), and calculates the hash (using the chosen algorithm) of the public key *K <sub>B</sub> <sup>public</sup>*.
7. Bob's device replies to Alice's device with an `m.key.verification.accept` message.
8. Alice's device receives Bob's message and stores the commitment hash for later use.
9. Alice's device creates an ephemeral Curve25519 key pair (*K <sub>A</sub> <sup>private</sup>*, *K <sub>A</sub> <sup>public</sup>*) and replies to Bob's device with an `m.key.verification.key`, sending only the public key *K <sub>A</sub> <sup>public</sup>*.
10. Bob's device receives Alice's message and replies with its own `m.key.verification.key` message containing its public key *K <sub>B</sub> <sup>public</sup>*.
11. Alice's device receives Bob's message and verifies the commitment hash from earlier matches the hash of the key Bob's device just sent and the content of Alice's `m.key.verification.start` message.
12. Both Alice's and Bob's devices perform an Elliptic-curve Diffie-Hellman using their private ephemeral key, and the other device's ephemeral public key (*ECDH(K <sub>A</sub> <sup>private</sup>*, *K <sub>B</sub> <sup>public</sup>*) for Alice's device and *ECDH(K <sub>B</sub> <sup>private</sup>*, *K <sub>A</sub> <sup>public</sup>*) for Bob's device), using the result as the shared secret.
13. Both Alice and Bob's devices display a SAS to their users, which is derived from the shared key using one of the methods in this section. If multiple SAS methods are available, clients should allow the users to select a method.
14. Alice and Bob compare the strings shown by their devices, and tell their devices if they match or not.
15. Assuming they match, Alice and Bob's devices each calculate Message Authentication Codes (MACs) for:
	- Each of the keys that they wish the other user to verify (usually their device ed25519 key and their master cross-signing key).
	- The complete list of key IDs that they wish the other user to verify.
	The MAC calculation is defined [below](https://spec.matrix.org/unstable/client-server-api/#mac-calculation).
16. Alice's device sends Bob's device an `m.key.verification.mac` message containing the MAC of Alice's device keys and the MAC of her key IDs to be verified. Bob's device does the same for Bob's device keys and key IDs concurrently with Alice.
17. When the other device receives the `m.key.verification.mac` message, the device calculates the MACs of its copies of the other device's keys given in the message, as well as the MAC of the comma-separated, sorted, list of key IDs in the message. The device compares these with the MAC values given in the message, and if everything matches then the device keys are verified.
18. Alice and Bob's devices send `m.key.verification.done` messages to complete the verification.

The wire protocol looks like the following between Alice and Bob's devices:

```
+-------------+                    +-----------+
    | AliceDevice |                    | BobDevice |
    +-------------+                    +-----------+
          |                                 |
          | m.key.verification.start        |
          |-------------------------------->|
          |                                 |
          |       m.key.verification.accept |
          |<--------------------------------|
          |                                 |
          | m.key.verification.key          |
          |-------------------------------->|
          |                                 |
          |          m.key.verification.key |
          |<--------------------------------|
          |                                 |
          | m.key.verification.mac          |
          |-------------------------------->|
          |                                 |
          |          m.key.verification.mac |
          |<--------------------------------|
          |                                 |
```

At any point the interactive verification can go wrong. The following describes what to do when an error happens:

- Alice or Bob can cancel the verification at any time. An `m.key.verification.cancel` message must be sent to signify the cancellation.
- The verification can time out. Clients should time out a verification that does not complete within 10 minutes. Additionally, clients should expire a `transaction_id` which goes unused for 10 minutes after having last sent/received it. The client should inform the user that the verification timed out, and send an appropriate `m.key.verification.cancel` message to the other device.
- When the same device attempts to initiate multiple verification attempts, the recipient should cancel all attempts with that device.
- When a device receives an unknown `transaction_id`, it should send an appropriate `m.key.verification.cancel` message to the other device indicating as such. This does not apply for inbound `m.key.verification.start` or `m.key.verification.cancel` messages.
- If the two devices do not share a common key share, hash, HMAC, or SAS method then the device should notify the other device with an appropriate `m.key.verification.cancel` message.
- If the user claims the Short Authentication Strings do not match, the device should send an appropriate `m.key.verification.cancel` message to the other device.
- If the device receives a message out of sequence or that it was not expecting, it should notify the other device with an appropriate `m.key.verification.cancel` message.

##### Verification messages specific to SAS

Building off the common framework, the following events are involved in SAS verification.

The `m.key.verification.cancel` event is unchanged, however the following error codes are used in addition to those already specified:

- `m.unknown_method`: The devices are unable to agree on the key agreement, hash, MAC, or SAS method.
- `m.mismatched_commitment`: The hash commitment did not match.
- `m.mismatched_sas`: The SAS did not match.

## m.key.verification.start with method: m.sas.v1

---

Begins a SAS key verification process using the `m.sas.v1` method.

| Event type: | Message event |
| --- | --- |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `from_device` | `string` | **Required:** The device ID which is initiating the process. |
| `hashes` | `[string]` | **Required:** The hash methods the sending device understands. Must include at least `sha256`. |
| `key_agreement_protocols` | `[string]` | **Required:** The key agreement protocols the sending device understands. Should include at least `curve25519-hkdf-sha256`. |
| `m.relates_to` | `[VerificationRelatesTo](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationstartmsasv1_verificationrelatesto)` | Required when sent as an in-room message. Indicates the `m.key.verification.request` that this message is related to. Note that for encrypted messages, this property should be in the unencrypted portion of the event. |
| `message_authentication_codes` | `[string]` | **Required:** The message authentication code methods that the sending device understands. Must include at least `hkdf-hmac-sha256.v2`. Should also include `hkdf-hmac-sha256` for compatibility with older clients, though this identifier is deprecated and will be removed in a future version of the spec. |
| `method` | `string` | **Required:** The verification method to use.  One of: `[m.sas.v1]`. |
| `short_authentication_string` | `[string]` | **Required:** The SAS methods the sending device (and the sending device's user) understands. Must include at least `decimal`. Optionally can include `emoji`. |
| `transaction_id` | `string` | Required when sent as a to-device message. An opaque identifier for the verification process. Must be unique with respect to the devices involved. Must be the same as the `transaction_id` given in the `m.key.verification.request` if this process is originating from a request. |

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | The event ID of the `m.key.verification.request` that this message is related to. |
| `rel_type` | `string` | The relationship type. Currently, this can only be an [`m.reference`](https://spec.matrix.org/unstable/client-server-api/#reference-relations) relationship type.  One of: `[m.reference]`. |

### Examples

```json
{
  "content": {
    "from_device": "BobDevice1",
    "hashes": [
      "sha256"
    ],
    "key_agreement_protocols": [
      "curve25519"
    ],
    "message_authentication_codes": [
      "hkdf-hmac-sha256.v2",
      "hkdf-hmac-sha256"
    ],
    "method": "m.sas.v1",
    "short_authentication_string": [
      "decimal",
      "emoji"
    ],
    "transaction_id": "S0meUniqueAndOpaqueString"
  },
  "type": "m.key.verification.start"
}
```

## m.key.verification.accept

---

Accepts a previously sent `m.key.verification.start` message.

| Event type: | Message event |
| --- | --- |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `commitment` | `string` | **Required:** The hash (encoded as unpadded base64) of the concatenation of the device's ephemeral public key (encoded as unpadded base64) and the canonical JSON representation of the `m.key.verification.start` message. |
| `hash` | `string` | **Required:** The hash method the device is choosing to use, out of the options in the `m.key.verification.start` message. |
| `key_agreement_protocol` | `string` | **Required:** The key agreement protocol the device is choosing to use, out of the options in the `m.key.verification.start` message. |
| `m.relates_to` | `[VerificationRelatesTo](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationaccept_verificationrelatesto)` | Required when sent as an in-room message. Indicates the `m.key.verification.request` that this message is related to. Note that for encrypted messages, this property should be in the unencrypted portion of the event. |
| `message_authentication_code` | `string` | **Required:** The message authentication code method the device is choosing to use, out of the options in the `m.key.verification.start` message. |
| `short_authentication_string` | `[string]` | **Required:** The SAS methods both devices involved in the verification process understand. Must be a subset of the options in the `m.key.verification.start` message. |
| `transaction_id` | `string` | Required when sent as a to-device message. An opaque identifier for the verification process. Must be the same as the one used for the `m.key.verification.start` message. |

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | The event ID of the `m.key.verification.request` that this message is related to. |
| `rel_type` | `string` | The relationship type. Currently, this can only be an [`m.reference`](https://spec.matrix.org/unstable/client-server-api/#reference-relations) relationship type.  One of: `[m.reference]`. |

### Examples

```json
{
  "content": {
    "commitment": "fQpGIW1Snz+pwLZu6sTy2aHy/DYWWTspTJRPyNp0PKkymfIsNffysMl6ObMMFdIJhk6g6pwlIqZ54rxo8SLmAg",
    "hash": "sha256",
    "key_agreement_protocol": "curve25519",
    "message_authentication_code": "hkdf-hmac-sha256.v2",
    "method": "m.sas.v1",
    "short_authentication_string": [
      "decimal",
      "emoji"
    ],
    "transaction_id": "S0meUniqueAndOpaqueString"
  },
  "type": "m.key.verification.accept"
}
```

## m.key.verification.key

---

Sends the ephemeral public key for a device to the partner device.

| Event type: | Message event |
| --- | --- |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `key` | `string` | **Required:** The device's ephemeral public key, encoded as unpadded base64. |
| `m.relates_to` | `[VerificationRelatesTo](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationkey_verificationrelatesto)` | Required when sent as an in-room message. Indicates the `m.key.verification.request` that this message is related to. Note that for encrypted messages, this property should be in the unencrypted portion of the event. |
| `transaction_id` | `string` | Required when sent as a to-device message. An opaque identifier for the verification process. Must be the same as the one used for the `m.key.verification.start` message. |

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | The event ID of the `m.key.verification.request` that this message is related to. |
| `rel_type` | `string` | The relationship type. Currently, this can only be an [`m.reference`](https://spec.matrix.org/unstable/client-server-api/#reference-relations) relationship type.  One of: `[m.reference]`. |

### Examples

```json
{
  "content": {
    "key": "fQpGIW1Snz+pwLZu6sTy2aHy/DYWWTspTJRPyNp0PKkymfIsNffysMl6ObMMFdIJhk6g6pwlIqZ54rxo8SLmAg",
    "transaction_id": "S0meUniqueAndOpaqueString"
  },
  "type": "m.key.verification.key"
}
```

## m.key.verification.mac

---

Sends the MAC of a device's key to the partner device. The MAC is calculated using the method given in `message_authentication_code` property of the `m.key.verification.accept` message.

| Event type: | Message event |
| --- | --- |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `keys` | `string` | **Required:** The MAC of the comma-separated, sorted, list of key IDs given in the `mac` property, encoded as unpadded base64. |
| `m.relates_to` | `[VerificationRelatesTo](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationmac_verificationrelatesto)` | Required when sent as an in-room message. Indicates the `m.key.verification.request` that this message is related to. Note that for encrypted messages, this property should be in the unencrypted portion of the event. |
| `mac` | `{string: string}` | **Required:** A map of the key ID to the MAC of the key, using the algorithm in the verification process. The MAC is encoded as unpadded base64. |
| `transaction_id` | `string` | Required when sent as a to-device message. An opaque identifier for the verification process. Must be the same as the one used for the `m.key.verification.start` message. |

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | The event ID of the `m.key.verification.request` that this message is related to. |
| `rel_type` | `string` | The relationship type. Currently, this can only be an [`m.reference`](https://spec.matrix.org/unstable/client-server-api/#reference-relations) relationship type.  One of: `[m.reference]`. |

### Examples

```json
{
  "content": {
    "keys": "2Wptgo4CwmLo/Y8B8qinxApKaCkBG2fjTWB7AbP5Uy+aIbygsSdLOFzvdDjww8zUVKCmI02eP9xtyJxc/cLiBA",
    "mac": {
      "ed25519:ABCDEF": "fQpGIW1Snz+pwLZu6sTy2aHy/DYWWTspTJRPyNp0PKkymfIsNffysMl6ObMMFdIJhk6g6pwlIqZ54rxo8SLmAg"
    },
    "transaction_id": "S0meUniqueAndOpaqueString"  }
}
```

###### MAC calculation

During the verification process, Message Authentication Codes (MACs) are calculated for keys and lists of key IDs.

The method used to calculate these MACs depends upon the value of the `message_authentication_code` property in the [`m.key.verification.accept`](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationaccept) message. All current implementations should use the `hkdf-hmac-sha256.v2` method which is defined as follows:

1. An HMAC key is generated using HKDF, as defined in [RFC 5869](https://tools.ietf.org/html/rfc5869), using SHA-256 as the hash function. The shared secret is supplied as the input keying material. No salt is used, and in the info parameter is the concatenation of:
	- The string `MATRIX_KEY_VERIFICATION_MAC`.
	- The Matrix ID of the user whose key is being MAC-ed.
	- The Device ID of the device sending the MAC.
	- The Matrix ID of the other user.
	- The Device ID of the device receiving the MAC.
	- The `transaction_id` being used.
	- The Key ID of the key being MAC-ed, or the string `KEY_IDS` if the item being MAC-ed is the list of key IDs.
2. A MAC is then generated using HMAC as defined in [RFC 2104](https://tools.ietf.org/html/rfc2104) with the key generated above and using SHA-256 as the hash function.
	If a key is being MACed, the MAC is performed on the public key as encoded according to the [key algorithm](https://spec.matrix.org/unstable/client-server-api/#key-algorithms). For example, for `ed25519` keys, it is the unpadded base64-encoded key.
	If the key list is being MACed, the list is sorted lexicographically and comma-separated with no extra whitespace added, with each key written in the form `{algorithm}:{keyId}`. For example, the key list could look like:`ed25519:Cross+Signing+Key,ed25519:DEVICEID`. In this way, the recipient can reconstruct the list from the names in the `mac` property of the `m.key.verification.mac` message and ensure that no keys were added or removed.
3. The MAC values are base64-encoded and sent in a [`m.key.verification.mac`](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationmac) message.

###### SAS HKDF calculation

In all of the SAS methods, HKDF is as defined in [RFC 5869](https://tools.ietf.org/html/rfc5869) and uses the previously agreed-upon hash function for the hash function. The shared secret is supplied as the input keying material. No salt is used. When the `key_agreement_protocol` is `curve25519-hkdf-sha256`, the info parameter is the concatenation of:

- The string `MATRIX_KEY_VERIFICATION_SAS|`.
- The Matrix ID of the user who sent the `m.key.verification.start` message, followed by `|`.
- The Device ID of the device which sent the `m.key.verification.start` message, followed by `|`.
- The public key from the `m.key.verification.key` message sent by the device which sent the `m.key.verification.start` message, encoded as unpadded base64, followed by `|`.
- The Matrix ID of the user who sent the `m.key.verification.accept` message, followed by `|`.
- The Device ID of the device which sent the `m.key.verification.accept` message, followed by `|`.
- The public key from the `m.key.verification.key` message sent by the device which sent the `m.key.verification.accept` message, encoded as unpadded base64, followed by `|`.
- The `transaction_id` being used.

When the `key_agreement_protocol` is the deprecated method `curve25519`, the info parameter is the concatenation of:

- The string `MATRIX_KEY_VERIFICATION_SAS`.
- The Matrix ID of the user who sent the `m.key.verification.start` message.
- The Device ID of the device which sent the `m.key.verification.start` message.
- The Matrix ID of the user who sent the `m.key.verification.accept` message.
- The Device ID of the device which sent the `m.key.verification.accept` message.
- The `transaction_id` being used.

New implementations are discouraged from implementing the `curve25519` method.

###### SAS method: decimal

Generate 5 bytes using [HKDF](https://spec.matrix.org/unstable/client-server-api/#sas-hkdf-calculation) then take sequences of 13 bits to convert to decimal numbers (resulting in 3 numbers between 0 and 8191 inclusive each). Add 1000 to each calculated number.

The bitwise operations to get the numbers given the 5 bytes *B <sub>0</sub>*, *B <sub>1</sub>*, *B <sub>2</sub>*, *B <sub>3</sub>*, *B <sub>4</sub>* would be:

- First: (*B <sub>0</sub>*  ≪ 5| *B <sub>1</sub>*  ≫ 3) + 1000
- Second: ((*B <sub>1</sub>* &0x7) ≪ 10| *B <sub>2</sub>*  ≪ 2| *B <sub>3</sub>*  ≫ 6) + 1000
- Third: ((*B <sub>3</sub>* &0x3F) ≪ 7| *B <sub>4</sub>*  ≫ 1) + 1000

The digits are displayed to the user either with an appropriate separator, such as dashes, or with the numbers on individual lines.

###### SAS method: emoji

Generate 6 bytes using [HKDF](https://spec.matrix.org/unstable/client-server-api/#sas-hkdf-calculation) then split the first 42 bits into 7 groups of 6 bits, similar to how one would base64 encode something. Convert each group of 6 bits to a number and use the following table to get the corresponding emoji:

| Number | Emoji | Unicode | Description |
| --- | --- | --- | --- |
| 0 | 🐶 | U+1F436 | Dog |
| 1 | 🐱 | U+1F431 | Cat |
| 2 | 🦁 | U+1F981 | Lion |
| 3 | 🐎 | U+1F40E | Horse |
| 4 | 🦄 | U+1F984 | Unicorn |
| 5 | 🐷 | U+1F437 | Pig |
| 6 | 🐘 | U+1F418 | Elephant |
| 7 | 🐰 | U+1F430 | Rabbit |
| 8 | 🐼 | U+1F43C | Panda |
| 9 | 🐓 | U+1F413 | Rooster |
| 10 | 🐧 | U+1F427 | Penguin |
| 11 | 🐢 | U+1F422 | Turtle |
| 12 | 🐟 | U+1F41F | Fish |
| 13 | 🐙 | U+1F419 | Octopus |
| 14 | 🦋 | U+1F98B | Butterfly |
| 15 | 🌷 | U+1F337 | Flower |
| 16 | 🌳 | U+1F333 | Tree |
| 17 | 🌵 | U+1F335 | Cactus |
| 18 | 🍄 | U+1F344 | Mushroom |
| 19 | 🌏 | U+1F30F | Globe |
| 20 | 🌙 | U+1F319 | Moon |
| 21 | ☁️ | U+2601U+FE0F | Cloud |
| 22 | 🔥 | U+1F525 | Fire |
| 23 | 🍌 | U+1F34C | Banana |
| 24 | 🍎 | U+1F34E | Apple |
| 25 | 🍓 | U+1F353 | Strawberry |
| 26 | 🌽 | U+1F33D | Corn |
| 27 | 🍕 | U+1F355 | Pizza |
| 28 | 🎂 | U+1F382 | Cake |
| 29 | ❤️ | U+2764U+FE0F | Heart |
| 30 | 😀 | U+1F600 | Smiley |
| 31 | 🤖 | U+1F916 | Robot |
| 32 | 🎩 | U+1F3A9 | Hat |
| 33 | 👓 | U+1F453 | Glasses |
| 34 | 🔧 | U+1F527 | Spanner |
| 35 | 🎅 | U+1F385 | Santa |
| 36 | 👍 | U+1F44D | Thumbs Up |
| 37 | ☂️ | U+2602U+FE0F | Umbrella |
| 38 | ⌛ | U+231B | Hourglass |
| 39 | ⏰ | U+23F0 | Clock |
| 40 | 🎁 | U+1F381 | Gift |
| 41 | 💡 | U+1F4A1 | Light Bulb |
| 42 | 📕 | U+1F4D5 | Book |
| 43 | ✏️ | U+270FU+FE0F | Pencil |
| 44 | 📎 | U+1F4CE | Paperclip |
| 45 | ✂️ | U+2702U+FE0F | Scissors |
| 46 | 🔒 | U+1F512 | Lock |
| 47 | 🔑 | U+1F511 | Key |
| 48 | 🔨 | U+1F528 | Hammer |
| 49 | ☎️ | U+260EU+FE0F | Telephone |
| 50 | 🏁 | U+1F3C1 | Flag |
| 51 | 🚂 | U+1F682 | Train |
| 52 | 🚲 | U+1F6B2 | Bicycle |
| 53 | ✈️ | U+2708U+FE0F | Aeroplane |
| 54 | 🚀 | U+1F680 | Rocket |
| 55 | 🏆 | U+1F3C6 | Trophy |
| 56 | ⚽ | U+26BD | Ball |
| 57 | 🎸 | U+1F3B8 | Guitar |
| 58 | 🎺 | U+1F3BA | Trumpet |
| 59 | 🔔 | U+1F514 | Bell |
| 60 | ⚓ | U+2693 | Anchor |
| 61 | 🎧 | U+1F3A7 | Headphones |
| 62 | 📁 | U+1F4C1 | Folder |
| 63 | 📌 | U+1F4CC | Pin |

Clients SHOULD show the emoji with the descriptions from the table, or appropriate translation of those descriptions. Client authors SHOULD collaborate to create a common set of translations for all languages.

##### Cross-signing

Rather than requiring Alice to verify each of Bob's devices with each of her own devices and vice versa, the cross-signing feature allows users to sign their device keys such that Alice and Bob only need to verify once. With cross-signing, each user has a set of cross-signing keys that are used to sign their own device keys and other users' keys, and can be used to trust device keys that were not verified directly.

Each user has three ed25519 key pairs for cross-signing:

- a master key (MSK) that serves as the user's identity in cross-signing and signs their other cross-signing keys;
- a user-signing key (USK) – only visible to the user that it belongs to –that signs other users' master keys; and
- a self-signing key (SSK) that signs the user's own device keys.

The master key may also be used to sign other items such as the backup key. The master key may also be signed by the user's own device keys to aid in migrating from device verifications: if Alice's device had previously verified Bob's device and Bob's device has signed his master key, then Alice's device can trust Bob's master key, and she can sign it with her user-signing key.

Users upload their cross-signing keys to the server using [POST /\_matrix/client/v3/keys/device\_signing/upload](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysdevice_signingupload). When Alice uploads new cross-signing keys, her user ID will appear in the `changed` property of the `device_lists` field of the `/sync` of response of all users who share an encrypted room with her. When Bob sees Alice's user ID in his `/sync`, he will call [POST /\_matrix/client/v3/keys/query](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysquery) to retrieve Alice's device and cross-signing keys.

If Alice has a device and wishes to send an encrypted message to Bob, she can trust Bob's device if:

- Alice's device is using a master key that has signed her user-signing key,
- Alice's user-signing key has signed Bob's master key,
- Bob's master key has signed Bob's self-signing key, and
- Bob's self-signing key has signed Bob's device key.

The following diagram illustrates how keys are signed:

```
+------------------+                ..................   +----------------+
    | +--------------+ |   ..................            :   | +------------+ |
    | |              v v   v            :   :            v   v v            | |
    | |           +-----------+         :   :         +-----------+         | |
    | |           | Alice MSK |         :   :         |  Bob MSK  |         | |
    | |           +-----------+         :   :         +-----------+         | |
    | |             |       :           :   :           :       |           | |
    | |          +--+       :...        :   :        ...:       +--+        | |
    | |          v             v        :   :        v             v        | |
    | |    +-----------+ .............  :   :  ............. +-----------+  | |
    | |    | Alice SSK | : Alice USK :  :   :  :  Bob USK  : |  Bob SSK  |  | |
    | |    +-----------+ :...........:  :   :  :...........: +-----------+  | |
    | |      |  ...  |         :        :   :        :         |  ...  |    | |
    | |      V       V         :........:   :........:         V       V    | |
    | | +---------+   -+                                  +---------+   -+  | |
    | | | Devices | ...|                                  | Devices | ...|  | |
    | | +---------+   -+                                  +---------+   -+  | |
    | |      |  ...  |                                         |  ...  |    | |
    | +------+       |                                         |       +----+ |
    +----------------+                                         +--------------+
```

In the diagram, boxes represent keys and lines represent signatures with the arrows pointing from the signing key to the key being signed. Dotted boxes and lines represent keys and signatures that are only visible to the user who created them.

The following diagram illustrates Alice's view, hiding the keys and signatures that she cannot see:

```
+------------------+                +----------------+   +----------------+
    | +--------------+ |                |                |   | +------------+ |
    | |              v v                |                v   v v            | |
    | |           +-----------+         |             +-----------+         | |
    | |           | Alice MSK |         |             |  Bob MSK  |         | |
    | |           +-----------+         |             +-----------+         | |
    | |             |       |           |                       |           | |
    | |          +--+       +--+        |                       +--+        | |
    | |          v             v        |                          v        | |
    | |    +-----------+ +-----------+  |                    +-----------+  | |
    | |    | Alice SSK | | Alice USK |  |                    |  Bob SSK  |  | |
    | |    +-----------+ +-----------+  |                    +-----------+  | |
    | |      |  ...  |         |        |                      |  ...  |    | |
    | |      V       V         +--------+                      V       V    | |
    | | +---------+   -+                                  +---------+   -+  | |
    | | | Devices | ...|                                  | Devices | ...|  | |
    | | +---------+   -+                                  +---------+   -+  | |
    | |      |  ...  |                                         |  ...  |    | |
    | +------+       |                                         |       +----+ |
    +----------------+                                         +--------------+
```

[Verification methods](https://spec.matrix.org/unstable/client-server-api/#device-verification) can be used to verify a user's master key by using the master public key, encoded using unpadded base64, as the device ID, and treating it as a normal device. For example, if Alice and Bob verify each other using SAS, Alice's `m.key.verification.mac` message to Bob may include `"ed25519:alices+master+public+key": "alices+master+public+key"` in the `mac` property. Servers therefore must ensure that device IDs will not collide with cross-signing public keys.

The cross-signing private keys can be stored on the server or shared with other devices using the [Secrets](https://spec.matrix.org/unstable/client-server-api/#secrets) module. When doing so, the master, user-signing, and self-signing keys are identified using the names `m.cross_signing.master`, `m.cross_signing.user_signing`, and `m.cross_signing.self_signing`, respectively, and the keys are base64-encoded before being encrypted.

###### Key and signature security

A user's master key could allow an attacker to impersonate that user to other users, or other users to that user. Thus clients must ensure that the private part of the master key is treated securely. If clients do not have a secure means of storing the master key (such as a secret storage system provided by the operating system), then clients must not store the private part.

If a user's client sees that any other user has changed their master key, that client must notify the user about the change before allowing communication between the users to continue.

Since device key IDs (`ed25519:DEVICE_ID`) and cross-signing key IDs (`ed25519:PUBLIC_KEY`) occupy the same namespace, clients must ensure that they use the correct keys when verifying.

While servers MUST not allow devices to have the same IDs as cross-signing keys, a malicious server could construct such a situation, so clients must not rely on the server being well-behaved and should take the following precautions against this.

1. Clients MUST refer to keys by their public keys during the verification process, rather than only by the key ID.
2. Clients MUST fix the keys that are being verified at the beginning of the verification process, and ensure that they do not change in the course of verification.
3. Clients SHOULD also display a warning and MUST refuse to verify a user when they detect that the user has a device with the same ID as a cross-signing key.

A user's user-signing and self-signing keys are intended to be easily replaceable if they are compromised by re-issuing a new key signed by the user's master key and possibly by re-verifying devices or users. However, doing so relies on the user being able to notice when their keys have been compromised, and it involves extra work for the user, and so although clients do not have to treat the private parts as sensitively as the master key, clients should still make efforts to store the private part securely, or not store it at all. Clients will need to balance the security of the keys with the usability of signing users and devices when performing key verification.

To avoid leaking of social graphs, servers will only allow users to see:

- signatures made by the user's own master, self-signing or user-signing keys,
- signatures made by the user's own devices about their own master key,
- signatures made by other users' self-signing keys about their respective devices,
- signatures made by other users' master keys about their respective self-signing key, or
- signatures made by other users' devices about their respective master keys.

Users will not be able to see signatures made by other users' user-signing keys.## POST /\_matrix/client/v3/keys/device\_signing/upload

---

**Added in `v1.1`**

**Changed in `v1.11`:** UIA is not always required for this endpoint.

Publishes cross-signing keys for the user.

This API endpoint uses the [User-Interactive Authentication API](https://spec.matrix.org/unstable/client-server-api/#user-interactive-authentication-api).

User-Interactive Authentication MUST be performed, except in these cases:

- there is no existing cross-signing master key uploaded to the homeserver, OR
- there is an existing cross-signing master key and it exactly matches the cross-signing master key provided in the request body. If there are any additional keys provided in the request (self-signing key, user-signing key) they MUST also match the existing keys stored on the server. In other words, the request contains no new keys.

This allows clients to freely upload one set of keys, but not modify/overwrite keys if they already exist. Allowing clients to upload the same set of keys more than once makes this endpoint idempotent in the case where the response is lost over the network, which would otherwise cause a UIA challenge upon retry.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `auth` | `[Authentication Data](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysdevice_signingupload_request_authentication-data)` | Additional authentication information for the user-interactive authentication API. |
| `master_key` | `[CrossSigningKey](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysdevice_signingupload_request_crosssigningkey)` | Optional. The user's master key. |
| `self_signing_key` | `[CrossSigningKey](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysdevice_signingupload_request_crosssigningkey)` | Optional. The user's self-signing key. Must be signed by the accompanying master key, or by the user's most recently uploaded master key if no master key is included in the request. |
| `user_signing_key` | `[CrossSigningKey](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysdevice_signingupload_request_crosssigningkey)` | Optional. The user's user-signing key. Must be signed by the accompanying master key, or by the user's most recently uploaded master key if no master key is included in the request. |

| Name | Type | Description |
| --- | --- | --- |
| `session` | `string` | The value of the session key given by the homeserver. |
| `type` | `string` | The authentication type that the client is attempting to complete. May be omitted if `session` is given, and the client is reissuing a request which it believes has been completed out-of-band (for example, via the [fallback mechanism](https://spec.matrix.org/unstable/client-server-api/#fallback)). |
| <Other properties> |  | Keys dependent on the login type |

| Name | Type | Description |
| --- | --- | --- |
| `keys` | `{string: string}` | **Required:** The public key. The object must have exactly one property, whose name is in the form `<algorithm>:<unpadded_base64_public_key>`, and whose value is the unpadded base64 public key. |
| `signatures` | `Signatures` | Signatures of the key, calculated using the process described at [Signing JSON](https://spec.matrix.org/unstable/appendices/#signing-json). Optional for the master key. Other keys must be signed by the user's master key. |
| `usage` | `[string]` | **Required:** What the key is used for. |
| `user_id` | `string` | **Required:** The ID of the user the key belongs to. |

### Request body example

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The provided keys were successfully uploaded. |
| `400` | The input was invalid in some way. This can include one of the following error codes:  - `M_INVALID_SIGNATURE`: For example, the self-signing or user-signing key had an incorrect signature. - `M_MISSING_PARAM`: No master key is available. |
| `403` | The public key of one of the keys is the same as one of the user's device IDs, or the request is not authorized for any other reason. |

### 200 response

```json
{}
```

### 400 response

```json
{

  "errcode": "M_INVALID_SIGNATURE",

  "error": "Invalid signature"

}
```

### 403 response

```json
{

  "errcode": "M_FORBIDDEN",

  "error": "Key ID in use"

}
```

## POST /\_matrix/client/v3/keys/signatures/upload

---

**Added in `v1.1`**

Publishes cross-signing signatures for the user.

The signed JSON object must match the key previously uploaded or retrieved for the given key ID, with the exception of the `signatures` property, which contains the new signature(s) to add.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request body

| Type | Description |
| --- | --- |
| `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): {string: object}}` | A map of user ID to a map of key ID to signed JSON object. |

### Request body example

```json
{

  "@alice:example.com": {

    "HIJKLMN": {

      "algorithms": [

        "m.olm.v1.curve25519-aes-sha256",

        "m.megolm.v1.aes-sha"

      ],

      "device_id": "HIJKLMN",

      "keys": {

        "curve25519:HIJKLMN": "base64+curve25519+key",

        "ed25519:HIJKLMN": "base64+ed25519+key"

      },

      "signatures": {

        "@alice:example.com": {

          "ed25519:base64+self+signing+public+key": "base64+signature+of+HIJKLMN"

        }

      },

      "user_id": "@alice:example.com"

    },

    "base64+master+public+key": {

      "keys": {

        "ed25519:base64+master+public+key": "base64+master+public+key"

      },

      "signatures": {

        "@alice:example.com": {

          "ed25519:HIJKLMN": "base64+signature+of+master+key"

        }

      },

      "usage": [

        "master"

      ],

      "user_id": "@alice:example.com"

    }

  },

  "@bob:example.com": {

    "bobs+base64+master+public+key": {

      "keys": {

        "ed25519:bobs+base64+master+public+key": "bobs+base64+master+public+key"

      },

      "signatures": {

        "@alice:example.com": {

          "ed25519:base64+user+signing+public+key": "base64+signature+of+bobs+master+key"

        }

      },

      "usage": [

        "master"

      ],

      "user_id": "@bob:example.com"

    }

  }

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The provided signatures were processed. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `failures` | `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): {string: Error}}` | A map from user ID to key ID to an error for any signatures that failed. If a signature was invalid, the `errcode` will be set to `M_INVALID_SIGNATURE`. |

```json
{

  "failures": {

    "@alice:example.com": {

      "HIJKLMN": {

        "errcode": "M_INVALID_SIGNATURE",

        "error": "Invalid signature"

      }

    }

  }

}
```

##### QR code verification

**\[Added in `v1.1`\]**

Verifying by QR codes provides a quick way to verify when one of the parties has a device capable of scanning a QR code. The QR code encodes both parties' master signing keys as well as a random shared secret that is used to allow bi-directional verification from a single scan.

To advertise the ability to show a QR code, clients use the names `m.qr_code.show.v1` and `m.reciprocate.v1` in the `methods` fields of the `m.key.verification.request` and `m.key.verification.ready` events. To advertise the ability to scan a QR code, clients use the names `m.qr_code.scan.v1` and `m.reciprocate.v1` in the `methods` fields of the `m.key.verification.request` and `m.key.verification.ready` events. Clients that support both showing and scanning QR codes would advertise `m.qr_code.show.v1`, `m.qr_code.scan.v1`, and `m.reciprocate.v1` as methods.

The process between Alice and Bob verifying each other would be:

1. Alice and Bob meet in person, and want to verify each other's keys.
2. Alice and Bob begin a key verification using the key verification framework as described above.
3. Alice's client displays a QR code that Bob is able to scan if Bob's client indicated the ability to scan, an option to scan Bob's QR code if her client is able to scan. Bob's client displays a QR code that Alice can scan if Alice's client indicated the ability to scan, and an option to scan Alice's QR code if his client is able to scan. The format for the QR code is described below. Other options, like starting SAS Emoji verification, can be presented alongside the QR code if the devices have appropriate support.
4. Alice scans Bob's QR code.
5. Alice's device ensures that the keys encoded in the QR code match the expected values for the keys. If not, Alice's device displays an error message indicating that the code is incorrect, and sends a `m.key.verification.cancel` message to Bob's device.
	Otherwise, at this point:
	- Alice's device has now verified Bob's key, and
	- Alice's device knows that Bob has the correct key for her.
	Thus for Bob to verify Alice's key, Alice needs to tell Bob that he has the right key.
6. Alice's device displays a message saying that the verification was successful because the QR code's keys will have matched the keys expected for Bob. Bob's device hasn't had a chance to verify Alice's keys yet so wouldn't show the same message. Bob will know that he has the right key for Alice because Alice's device will have shown this message, as otherwise the verification would be cancelled.
7. Alice's device sends an `m.key.verification.start` message with `method` set to `m.reciprocate.v1` to Bob (see below). The message includes the shared secret from the QR code. This signals to Bob's device that Alice has scanned Bob's QR code.
	This message is merely a signal for Bob's device to proceed to the next step, and is not used for verification purposes.
8. Upon receipt of the `m.key.verification.start` message, Bob's device ensures that the shared secret matches.
	If the shared secret does not match, it should display an error message indicating that an attack was attempted. (This does not affect Alice's verification of Bob's keys.)
	If the shared secret does match, it asks Bob to confirm that Alice has scanned the QR code.
9. Bob sees Alice's device confirm that the key matches, and presses the button on his device to indicate that Alice's key is verified.
	Bob's verification of Alice's key hinges on Alice telling Bob the result of her scan. Since the QR code includes what Bob thinks Alice's key is, Alice's device can check whether Bob has the right key for her. Alice has no motivation to lie about the result, as getting Bob to trust an incorrect key would only affect communications between herself and Bob. Thus Alice telling Bob that the code was scanned successfully is sufficient for Bob to trust Alice's key, under the assumption that this communication is done over a trusted medium (such as in-person).
10. Both devices send an `m.key.verification.done` message.

The QR codes to be displayed and scanned MUST be compatible with [ISO/IEC 18004:2015](https://www.iso.org/standard/62021.html) and contain a single segment that uses the byte mode encoding.

The error correction level can be chosen by the device displaying the QR code.

The binary segment MUST be of the following form:

- the string `MATRIX` encoded as one ASCII byte per character (i.e. `0x4D`,`0x41`, `0x54`, `0x52`, `0x49`, `0x58`)
- one byte indicating the QR code version (must be `0x02`)
- one byte indicating the QR code verification mode. Should be one of the following values:
	- `0x00` verifying another user with cross-signing
	- `0x01` self-verifying in which the current device does trust the master key
	- `0x02` self-verifying in which the current device does not yet trust the master key
- the event ID or `transaction_id` of the associated verification request event, encoded as:
	- two bytes in network byte order (big-endian) indicating the length in bytes of the ID as a UTF-8 string
	- the ID encoded as a UTF-8 string
- the first key, as 32 bytes. The key to use depends on the mode field:
	- if `0x00` or `0x01`, then the current user's own master cross-signing public key
	- if `0x02`, then the current device's Ed25519 signing key
- the second key, as 32 bytes. The key to use depends on the mode field:
	- if `0x00`, then what the device thinks the other user's master cross-signing public key is
	- if `0x01`, then what the device thinks the other device's Ed25519 signing public key is
	- if `0x02`, then what the device thinks the user's master cross-signing public key is
- a random shared secret, as a sequence of bytes. It is suggested to use a secret that is about 8 bytes long. Note: as we do not share the length of the secret, and it is not a fixed size, clients will just use the remainder of binary segment as the shared secret.

For example, if Alice displays a QR code encoding the following binary data:

```
"MATRIX"    |ver|mode| len   | event ID
 4D 41 54 52 49 58  02  00   00 2D   21 41 42 43 44 ...
| user's cross-signing key    | other user's cross-signing key | shared secret
  00 01 02 03 04 05 06 07 ...   10 11 12 13 14 15 16 17 ...      20 21 22 23 24 25 26 27
```

this indicates that Alice is verifying another user (say Bob), in response to the request from event "$ABCD…", her cross-signing key is `0001020304050607...` (which is "AAECAwQFBg…" in base64), she thinks that Bob's cross-signing key is `1011121314151617...` (which is "EBESExQVFh…" in base64), and the shared secret is `2021222324252627` (which is "ICEiIyQlJic" in base64).

## m.key.verification.start with method: m.reciprocate.v1

---

Begins a key verification process using the `m.reciprocate.v1` method, after scanning a QR code.

| Event type: | Message event |
| --- | --- |

## Content

| Name | Type | Description |
| --- | --- | --- |
| `from_device` | `string` | **Required:** The device ID which is initiating the process. |
| `m.relates_to` | `[VerificationRelatesTo](https://spec.matrix.org/unstable/client-server-api/#mkeyverificationstartmreciprocatev1_verificationrelatesto)` | Required when sent as an in-room message. Indicates the `m.key.verification.request` that this message is related to. Note that for encrypted messages, this property should be in the unencrypted portion of the event. |
| `method` | `string` | **Required:** The verification method to use.  One of: `[m.reciprocate.v1]`. |
| `secret` | `string` | **Required:** The shared secret from the QR code, encoded using unpadded base64. |
| `transaction_id` | `string` | Required when sent as a to-device message. An opaque identifier for the verification process. Must be unique with respect to the devices involved. Must be the same as the `transaction_id` given in the `m.key.verification.request` if this process is originating from a request. |

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | The event ID of the `m.key.verification.request` that this message is related to. |
| `rel_type` | `string` | The relationship type. Currently, this can only be an [`m.reference`](https://spec.matrix.org/unstable/client-server-api/#reference-relations) relationship type.  One of: `[m.reference]`. |

## Examples

#### Sharing keys between devices

If Bob has an encrypted conversation with Alice on his computer, and then logs in through his phone for the first time, he may want to have access to the previously exchanged messages. To address this issue, several methods are provided to allow users to transfer keys from one device to another.

##### Key requests

When a device is missing keys to decrypt messages, it can request the keys by sending [m.room\_key\_request](https://spec.matrix.org/unstable/client-server-api/#mroom_key_request) to-device messages to other devices with `action` set to `request`.

If a device wishes to share the keys with that device, it can forward the keys to the first device by sending an encrypted [m.forwarded\_room\_key](https://spec.matrix.org/unstable/client-server-api/#mforwarded_room_key) to-device message. The first device should then send an [m.room\_key\_request](https://spec.matrix.org/unstable/client-server-api/#mroom_key_request) to-device message with `action` set to `request_cancellation` to the other devices that it had originally sent the key request to; a device that receives a `request_cancellation` should disregard any previously-received `request` message with the same `request_id` and `requesting_device_id`.

If a device does not wish to share keys with that device, it can indicate this by sending an [m.room\_key.withheld](https://spec.matrix.org/unstable/client-server-api/#mroom_keywithheld) to-device message, as described in [Reporting that decryption keys are withheld](https://spec.matrix.org/unstable/client-server-api/#reporting-that-decryption-keys-are-withheld).

##### Server-side key backups

Devices may upload encrypted copies of keys to the server. When a device tries to read a message that it does not have keys for, it may request the key from the server and decrypt it. Backups are per-user, and users may replace backups with new backups.

In contrast with [key requests](https://spec.matrix.org/unstable/client-server-api/#key-requests), server-side key backups do not require another device to be online from which to request keys. However, as the session keys are stored on the server encrypted, the client requires a [decryption key](https://spec.matrix.org/unstable/client-server-api/#decryption-key) to decrypt the session keys.

To create a backup, a client will call [POST /\_matrix/client/v3/room\_keys/version](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3room_keysversion) and define how the keys are to be encrypted through the backup's `auth_data`; other clients can discover the backup by calling [GET /\_matrix/client/v3/room\_keys/version](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3room_keysversion). Keys are encrypted according to the backup's `auth_data` and added to the backup by calling [PUT /\_matrix/client/v3/room\_keys/keys](https://spec.matrix.org/unstable/client-server-api/#put_matrixclientv3room_keyskeys) or one of its variants, and can be retrieved by calling [GET /\_matrix/client/v3/room\_keys/keys](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3room_keyskeys) or one of its variants. Keys can only be written to the most recently created version of the backup. Backups can also be deleted using [DELETE /\_matrix/client/v3/room\_keys/version/{version}](https://spec.matrix.org/unstable/client-server-api/#delete_matrixclientv3room_keysversionversion), or individual keys can be deleted using [DELETE /\_matrix/client/v3/room\_keys/keys](https://spec.matrix.org/unstable/client-server-api/#delete_matrixclientv3room_keyskeys) or one of its variants.

Clients must only store keys in backups after they have ensured that the `auth_data` is trusted. This can be done either by:

- checking that it is signed by the user's [master cross-signing key](https://spec.matrix.org/unstable/client-server-api/#cross-signing) or by a verified device belonging to the same user, or
- deriving the public key from a private key that it obtained from a trusted source. Trusted sources for the private key include the user entering the key, retrieving the key stored in [secret storage](https://spec.matrix.org/unstable/client-server-api/#secret-storage), or obtaining the key via [secret sharing](https://spec.matrix.org/unstable/client-server-api/#sharing) from a verified device belonging to the same user.

When a client uploads a key for a session that the server already has a key for, the server will choose to either keep the existing key or replace it with the new key based on the key metadata as follows:

- if the keys have different values for `is_verified`, then it will keep the key that has `is_verified` set to `true`;
- if they have the same values for `is_verified`, then it will keep the key with a lower `first_message_index`;
- and finally, if `is_verified` and `first_message_index` are equal, then it will keep the key with a lower `forwarded_count`.

###### Decryption key

Normally, the decryption key (i.e. the secret part of the encryption key) is stored on the server or shared with other devices using the [Secrets](https://spec.matrix.org/unstable/client-server-api/#secrets) module. When doing so, it is identified using the name `m.megolm_backup.v1`, and the key is base64-encoded before being encrypted.

If the backup decryption key is given directly to the user, the key should be presented as a string using the common [cryptographic key representation](https://spec.matrix.org/unstable/appendices/#cryptographic-key-representation).

###### Backup algorithm: m.megolm\_backup.v1.curve25519-aes-sha2

When a backup is created with the `algorithm` set to `m.megolm_backup.v1.curve25519-aes-sha2`, the `auth_data` should have the following format:

## AuthData

---

The format of the `auth_data` when a key backup is created with the `algorithm` set to `m.megolm_backup.v1.curve25519-aes-sha2`.

| Name | Type | Description |
| --- | --- | --- |
| `public_key` | `string` | **Required:** The curve25519 public key used to encrypt the backups, encoded in unpadded base64. |
| `signatures` | `object` | Signatures of the `auth_data`, as Signed JSON |

## Examples

```json
{

  "public_key": "abcdefg",

  "signatures": {

    "something": {

      "ed25519:something": "hijklmnop"

    }

  }

}
```

The `session_data` field in the backups is constructed as follows:

1. Encode the session key to be backed up as a JSON object using the `BackedUpSessionData` format defined below.
2. Generate an ephemeral curve25519 key, and perform an ECDH with the ephemeral key and the backup's public key to generate a shared secret. The public half of the ephemeral key, encoded using unpadded base64, becomes the `ephemeral` property of the `session_data`.
3. Using the shared secret, generate 80 bytes by performing an HKDF using SHA-256 as the hash, with a salt of 32 bytes of 0, and with the empty string as the info. The first 32 bytes are used as the AES key, the next 32 bytes are used as the MAC key, and the last 16 bytes are used as the AES initialization vector.
4. Stringify the JSON object, and encrypt it using AES-CBC-256 with PKCS#7 padding. This encrypted data, encoded using unpadded base64, becomes the `ciphertext` property of the `session_data`.
5. Pass an empty string through HMAC-SHA-256 using the MAC key generated above. The first 8 bytes of the resulting MAC are base64-encoded, and become the `mac` property of the `session_data`.

## BackedUpSessionData

---

The format of a backed-up session key, prior to encryption, when using the `m.megolm_backup.v1.curve25519-aes-sha2` algorithm.

| Name | Type | Description |
| --- | --- | --- |
| `algorithm` | `string` | **Required:** The end-to-end message encryption algorithm that the key is for. Must be `m.megolm.v1.aes-sha2`. |
| `forwarding_curve25519_key_chain` | `[string]` | **Required:** Chain of Curve25519 keys through which this session was forwarded, via [m.forwarded\_room\_key](https://spec.matrix.org/unstable/client-server-api/#mforwarded_room_key) events. |
| `sender_claimed_keys` | `{string: string}` | **Required:** A map from algorithm name (`ed25519`) to the Ed25519 signing key of the sending device. |
| `sender_key` | `string` | **Required:** Unpadded base64-encoded device Curve25519 key. |
| `session_key` | `string` | **Required:** Unpadded base64-encoded session key in [session-export format](https://gitlab.matrix.org/matrix-org/olm/blob/master/docs/megolm.md#session-export-format). |

## Examples

```json
{

  "algorithm": "m.megolm.v1.aes-sha2",

  "forwarding_curve25519_key_chain": [

    "hPQNcabIABgGnx3/ACv/jmMmiQHoeFfuLB17tzWp6Hw"

  ],

  "sender_claimed_keys": {

    "ed25519": "aj40p+aw64yPIdsxoog8jhPu9i7l7NcFRecuOQblE3Y"

  },

  "sender_key": "RF3s+E7RkTQTGF2d8Deol0FkQvgII2aJDf3/Jp5mxVU",

  "session_key": "AgAAAADxKHa9uFxcXzwYoNueL5Xqi69IkD4sni8Llf..."

}
```## GET /\_matrix/client/v3/room\_keys/keys

---

**Added in `v1.1`**

Retrieve the keys from the backup.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `version` | `string` | **Required:** The backup from which to retrieve the keys. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The key data. If no keys are found, then an object with an empty `rooms` property will be returned (`{"rooms": {}}`). |
| `404` | The backup was not found. |
| `429` | This request was rate-limited. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `rooms` | `{[Room ID](https://spec.matrix.org/unstable/appendices#room-ids): [RoomKeyBackup](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3room_keyskeys_response-200_roomkeybackup)}` | **Required:** A map of room IDs to room key backup data. |

| Name | Type | Description |
| --- | --- | --- |
| `sessions` | `{string: [KeyBackupData](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3room_keyskeys_response-200_keybackupdata)}` | **Required:** A map of session IDs to key data. |

| Name | Type | Description |
| --- | --- | --- |
| `first_message_index` | `integer` | **Required:** The index of the first message in the session that the key can decrypt. |
| `forwarded_count` | `integer` | **Required:** The number of times this key has been forwarded via key-sharing between devices. |
| `is_verified` | `boolean` | **Required:** Whether the device backing up the key verified the device that the key is from. |
| `session_data` | `object` | **Required:** Algorithm-dependent data. See the documentation for the backup algorithms in [Server-side key backups](https://spec.matrix.org/unstable/client-server-api/#server-side-key-backups) for more information on the expected format of the data. |

```json
{

  "rooms": {

    "!room:example.org": {

      "sessions": {

        "sessionid1": {

          "first_message_index": 1,

          "forwarded_count": 0,

          "is_verified": true,

          "session_data": {

            "ciphertext": "base64+ciphertext+of+JSON+data",

            "ephemeral": "base64+ephemeral+key",

            "mac": "base64+mac+of+ciphertext"

          }

        }

      }

    }

  }

}
```

### 404 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "Unknown backup version."

}
```

### 429 response

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

## PUT /\_matrix/client/v3/room\_keys/keys

---

**Added in `v1.1`**

Store several keys in the backup.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `version` | `string` | **Required:** The backup in which to store the keys. Must be the current backup. |

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `rooms` | `{[Room ID](https://spec.matrix.org/unstable/appendices#room-ids): [RoomKeyBackup](https://spec.matrix.org/unstable/client-server-api/#put_matrixclientv3room_keyskeys_request_roomkeybackup)}` | **Required:** A map of room IDs to room key backup data. |

| Name | Type | Description |
| --- | --- | --- |
| `sessions` | `{string: [KeyBackupData](https://spec.matrix.org/unstable/client-server-api/#put_matrixclientv3room_keyskeys_request_keybackupdata)}` | **Required:** A map of session IDs to key data. |

| Name | Type | Description |
| --- | --- | --- |
| `first_message_index` | `integer` | **Required:** The index of the first message in the session that the key can decrypt. |
| `forwarded_count` | `integer` | **Required:** The number of times this key has been forwarded via key-sharing between devices. |
| `is_verified` | `boolean` | **Required:** Whether the device backing up the key verified the device that the key is from. |
| `session_data` | `object` | **Required:** Algorithm-dependent data. See the documentation for the backup algorithms in [Server-side key backups](https://spec.matrix.org/unstable/client-server-api/#server-side-key-backups) for more information on the expected format of the data. |

### Request body example

```json
{

  "rooms": {

    "!room:example.org": {

      "sessions": {

        "sessionid1": {

          "first_message_index": 1,

          "forwarded_count": 0,

          "is_verified": true,

          "session_data": {

            "ciphertext": "base64+ciphertext+of+JSON+data",

            "ephemeral": "base64+ephemeral+key",

            "mac": "base64+mac+of+ciphertext"

          }

        }

      }

    }

  }

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The update succeeded |
| `403` | The version specified does not match the current backup version. The current version will be included in the `current_version` field. |
| `404` | The backup was not found. |
| `429` | This request was rate-limited. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `count` | `integer` | **Required:** The number of keys stored in the backup |
| `etag` | `string` | **Required:** The new etag value representing stored keys in the backup.  See [`GET /room_keys/version/{version}`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3room_keysversionversion) for more details. |

```json
{

  "count": 10,

  "etag": "abcdefg"

}
```

### 403 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "current_version": "42",

  "errcode": "M_WRONG_ROOM_KEYS_VERSION",

  "error": "Wrong backup version."

}
```

### 404 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "Unknown backup version"

}
```

### 429 response

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

## DELETE /\_matrix/client/v3/room\_keys/keys

---

**Added in `v1.1`**

Delete the keys from the backup.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `version` | `string` | **Required:** The backup from which to delete the key |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The update succeeded |
| `404` | The backup was not found. |
| `429` | This request was rate-limited. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `count` | `integer` | **Required:** The number of keys stored in the backup |
| `etag` | `string` | **Required:** The new etag value representing stored keys in the backup.  See [`GET /room_keys/version/{version}`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3room_keysversionversion) for more details. |

```json
{

  "count": 10,

  "etag": "abcdefg"

}
```

### 404 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "Unknown backup version"

}
```

### 429 response

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

## GET /\_matrix/client/v3/room\_keys/keys/{roomId}

---

**Added in `v1.1`**

Retrieve the keys from the backup for a given room.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The ID of the room that the requested key is for. |

| Name | Type | Description |
| --- | --- | --- |
| `version` | `string` | **Required:** The backup from which to retrieve the key. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The key data. If no keys are found, then an object with an empty `sessions` property will be returned (`{"sessions": {}}`). |
| `404` | The backup was not found. |
| `429` | This request was rate-limited. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `sessions` | `{string: [KeyBackupData](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3room_keyskeysroomid_response-200_keybackupdata)}` | **Required:** A map of session IDs to key data. |

| Name | Type | Description |
| --- | --- | --- |
| `first_message_index` | `integer` | **Required:** The index of the first message in the session that the key can decrypt. |
| `forwarded_count` | `integer` | **Required:** The number of times this key has been forwarded via key-sharing between devices. |
| `is_verified` | `boolean` | **Required:** Whether the device backing up the key verified the device that the key is from. |
| `session_data` | `object` | **Required:** Algorithm-dependent data. See the documentation for the backup algorithms in [Server-side key backups](https://spec.matrix.org/unstable/client-server-api/#server-side-key-backups) for more information on the expected format of the data. |

```json
{

  "sessions": {

    "sessionid1": {

      "first_message_index": 1,

      "forwarded_count": 0,

      "is_verified": true,

      "session_data": {

        "ciphertext": "base64+ciphertext+of+JSON+data",

        "ephemeral": "base64+ephemeral+key",

        "mac": "base64+mac+of+ciphertext"

      }

    }

  }

}
```

### 404 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "Unknown backup version"

}
```

### 429 response

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

## PUT /\_matrix/client/v3/room\_keys/keys/{roomId}

---

**Added in `v1.1`**

Store several keys in the backup for a given room.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The ID of the room that the keys are for. |

| Name | Type | Description |
| --- | --- | --- |
| `version` | `string` | **Required:** The backup in which to store the keys. Must be the current backup. |

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `sessions` | `{string: [KeyBackupData](https://spec.matrix.org/unstable/client-server-api/#put_matrixclientv3room_keyskeysroomid_request_keybackupdata)}` | **Required:** A map of session IDs to key data. |

| Name | Type | Description |
| --- | --- | --- |
| `first_message_index` | `integer` | **Required:** The index of the first message in the session that the key can decrypt. |
| `forwarded_count` | `integer` | **Required:** The number of times this key has been forwarded via key-sharing between devices. |
| `is_verified` | `boolean` | **Required:** Whether the device backing up the key verified the device that the key is from. |
| `session_data` | `object` | **Required:** Algorithm-dependent data. See the documentation for the backup algorithms in [Server-side key backups](https://spec.matrix.org/unstable/client-server-api/#server-side-key-backups) for more information on the expected format of the data. |

### Request body example

```json
{

  "sessions": {

    "sessionid1": {

      "first_message_index": 1,

      "forwarded_count": 0,

      "is_verified": true,

      "session_data": {

        "ciphertext": "base64+ciphertext+of+JSON+data",

        "ephemeral": "base64+ephemeral+key",

        "mac": "base64+mac+of+ciphertext"

      }

    }

  }

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The update succeeded |
| `403` | The version specified does not match the current backup version. The current version will be included in the `current_version` field. |
| `404` | The backup was not found. |
| `429` | This request was rate-limited. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `count` | `integer` | **Required:** The number of keys stored in the backup |
| `etag` | `string` | **Required:** The new etag value representing stored keys in the backup.  See [`GET /room_keys/version/{version}`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3room_keysversionversion) for more details. |

```json
{

  "count": 10,

  "etag": "abcdefg"

}
```

### 403 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "current_version": "42",

  "errcode": "M_WRONG_ROOM_KEYS_VERSION",

  "error": "Wrong backup version."

}
```

### 404 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "errcode": "M_NOT_FOUND",

  "error": "Unknown backup version"

}
```

### 429 response

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