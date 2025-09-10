# Matrix Server-Server API: End-to-End Encryption

**Section 27 of the Matrix Server-Server API specification**

This section covers end-to-end encryption federation capabilities in the Matrix Server-Server API, including key claiming, key querying, and cross-signing key updates.

---

## End-to-End Encryption

This section complements the [End-to-End Encryption module](https://spec.matrix.org/unstable/client-server-api/#end-to-end-encryption) of the Client-Server API. For detailed information about end-to-end encryption, please see that module.

The APIs defined here are designed to be able to proxy much of the client's request through to federation, and have the response also be proxied through to the client.

## POST /\_matrix/federation/v1/user/keys/claim

---

Claims one-time keys for use in pre-key messages.

The request contains the user ID, device ID and algorithm name of the keys that are required. If a key matching these requirements can be found, the response contains it. The returned key is a one-time key if one is available, and otherwise a fallback key.

One-time keys are given out in the order that they were uploaded via [/keys/upload](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3keysupload). (All keys uploaded within a given call to `/keys/upload` are considered equivalent in this regard; no ordering is specified within them.)

Servers must ensure that each one-time key is returned at most once, so when a key has been returned, no other request will ever return the same key.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `one_time_keys` | `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): {string: string}}` | **Required:** The keys to be claimed. A map from user ID, to a map from device ID to algorithm name. Requested users must be local to the receiving homeserver. |

### Request body example

```json
{

  "one_time_keys": {

    "@alice:example.com": {

      "JLAFKJWSCS": "signed_curve25519"

    }

  }

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The claimed keys. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `one_time_keys` | `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): {string: {string: string\|[KeyObject](https://spec.matrix.org/unstable/server-server-api/#post_matrixfederationv1userkeysclaim_response-200_keyobject)}}}` | **Required:** One-time keys for the queried devices. A map from user ID, to a map from devices to a map from `<algorithm>:<key_id>` to the key object.  See the [Client-Server Key Algorithms](https://spec.matrix.org/unstable/client-server-api/#key-algorithms) section for more information on the Key Object format. |

| Name | Type | Description |
| --- | --- | --- |
| `key` | `string` | **Required:** The key, encoded using unpadded base64. |
| `signatures` | `{string: {string: string}}` | **Required:** Signature of the key object.  The signature is calculated using the process described at [Signing JSON](https://spec.matrix.org/unstable/appendices/#signing-json). |

```json
{

  "one_time_keys": {

    "@alice:example.com": {

      "JLAFKJWSCS": {

        "signed_curve25519:AAAAHg": {

          "key": "zKbLg+NrIjpnagy+pIY6uPL4ZwEG2v+8F9lmgsnlZzs",

          "signatures": {

            "@alice:example.com": {

              "ed25519:JLAFKJWSCS": "FLWxXqGbwrb8SM3Y795eB6OA8bwBcoMZFXBqnTn58AYWZSqiD45tlBVcDa2L7RwdKXebW/VzDlnfVJ+9jok1Bw"

            }

          }

        }

      }

    }

  }

}
```

## POST /\_matrix/federation/v1/user/keys/query

---

Returns the current devices and identity keys for the given users.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `device_keys` | `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): [string]}` | **Required:** The keys to be downloaded. A map from user ID, to a list of device IDs, or to an empty list to indicate all devices for the corresponding user. Requested users must be local to the receiving homeserver. |

### Request body example

```json
{

  "device_keys": {

    "@alice:example.com": []

  }

}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The device information. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `device_keys` | `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): {string: [DeviceKeys](https://spec.matrix.org/unstable/server-server-api/#post_matrixfederationv1userkeysquery_response-200_devicekeys)}}` | **Required:** Information on the queried devices. A map from user ID, to a map from device ID to device information. For each device, the information returned will be the same as uploaded via `/keys/upload`, with the addition of an `unsigned` property. |
| `master_keys` | `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): [CrossSigningKey](https://spec.matrix.org/unstable/server-server-api/#post_matrixfederationv1userkeysquery_response-200_crosssigningkey)}` | Information on the master cross-signing keys of the queried users. A map from user ID, to master key information. For each key, the information returned will be the same as uploaded via `/keys/device_signing/upload`, along with the signatures uploaded via `/keys/signatures/upload` that the user is allowed to see.  **Added in `v1.1`** |
| `self_signing_keys` | `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): [CrossSigningKey](https://spec.matrix.org/unstable/server-server-api/#post_matrixfederationv1userkeysquery_response-200_crosssigningkey)}` | Information on the self-signing keys of the queried users. A map from user ID, to self-signing key information. For each key, the information returned will be the same as uploaded via `/keys/device_signing/upload`.  **Added in `v1.1`** |

| Name | Type | Description |
| --- | --- | --- |
| `algorithms` | `[string]` | **Required:** The encryption algorithms supported by this device. |
| `device_id` | `string` | **Required:** The ID of the device these keys belong to. Must match the device ID used when logging in. |
| `keys` | `{string: string}` | **Required:** Public identity keys. The names of the properties should be in the format `<algorithm>:<device_id>`. The keys themselves should be encoded as specified by the key algorithm. |
| `signatures` | `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): {string: string}}` | **Required:** Signatures for the device key object. A map from user ID, to a map from `<algorithm>:<device_id>` to the signature.  The signature is calculated using the process described at [Signing JSON](https://spec.matrix.org/unstable/appendices/#signing-json). |
| `unsigned` | `[UnsignedDeviceInfo](https://spec.matrix.org/unstable/server-server-api/#post_matrixfederationv1userkeysquery_response-200_unsigneddeviceinfo)` | Additional data added to the device key information by intermediate servers, and not covered by the signatures. |
| `user_id` | `string` | **Required:** The ID of the user the device belongs to. Must match the user ID used when logging in. |

| Name | Type | Description |
| --- | --- | --- |
| `device_display_name` | `string` | The display name which the user set on the device. |

| Name | Type | Description |
| --- | --- | --- |
| `keys` | `{string: string}` | **Required:** The public key. The object must have exactly one property, whose name is in the form `<algorithm>:<unpadded_base64_public_key>`, and whose value is the unpadded base64 public key. |
| `signatures` | `Signatures` | Signatures of the key, calculated using the process described at [Signing JSON](https://spec.matrix.org/unstable/appendices/#signing-json). Optional for the master key. Other keys must be signed by the user's master key. |
| `usage` | `[string]` | **Required:** What the key is used for. |
| `user_id` | `string` | **Required:** The ID of the user the key belongs to. |

```json
{

  "device_keys": {

    "@alice:example.com": {

      "JLAFKJWSCS": {

        "algorithms": [

          "m.olm.v1.curve25519-aes-sha2",

          "m.megolm.v1.aes-sha2"

        ],

        "device_id": "JLAFKJWSCS",

        "keys": {

          "curve25519:JLAFKJWSCS": "3C5BFWi2Y8MaVvjM8M22DBmh24PmgR0nPvJOIArzgyI",

          "ed25519:JLAFKJWSCS": "lEuiRJBit0IG6nUf5pUzWTUEsRVVe/HJkoKuEww9ULI"

        },

        "signatures": {

          "@alice:example.com": {

            "ed25519:JLAFKJWSCS": "dSO80A01XiigH3uBiDVx/EjzaoycHcjq9lfQX0uWsqxl2giMIiSPR8a4d291W1ihKJL/a+myXS367WT6NAIcBA"

          }

        },

        "unsigned": {

          "device_display_name": "Alice's mobile phone"

        },

        "user_id": "@alice:example.com"

      }

    }

  }

}
```

## m.signing\_key\_update

---

**Added in `v1.1`**

An EDU that lets servers push details to each other when one of their users updates their cross-signing keys.

| Name | Type | Description |
| --- | --- | --- |
| `content` | `[Signing Key Update](https://spec.matrix.org/unstable/server-server-api/#definition-msigning_key_update_signing-key-update)` | **Required:** The updated signing keys. |
| `edu_type` | `string` | **Required:** The string `m.signing_update`.  One of: `[m.signing_key_update]`. |

| Name | Type | Description |
| --- | --- | --- |
| `master_key` | `[CrossSigningKey](https://spec.matrix.org/unstable/server-server-api/#definition-msigning_key_update_crosssigningkey)` | Cross signing key |
| `self_signing_key` | `[CrossSigningKey](https://spec.matrix.org/unstable/server-server-api/#definition-msigning_key_update_crosssigningkey)` | Cross signing key |
| `user_id` | `string` | **Required:** The user ID whose cross-signing keys have changed. |

| Name | Type | Description |
| --- | --- | --- |
| `keys` | `{string: string}` | **Required:** The public key. The object must have exactly one property, whose name is in the form `<algorithm>:<unpadded_base64_public_key>`, and whose value is the unpadded base64 public key. |
| `signatures` | `Signatures` | Signatures of the key, calculated using the process described at [Signing JSON](https://spec.matrix.org/unstable/appendices/#signing-json). Optional for the master key. Other keys must be signed by the user's master key. |
| `usage` | `[string]` | **Required:** What the key is used for. |
| `user_id` | `string` | **Required:** The ID of the user the key belongs to. |

## Examples

```json
{

  "content": {

    "master_key": {

      "keys": {

        "ed25519:base64+master+public+key": "base64+master+public+key"

      },

      "usage": [

        "master"

      ],

      "user_id": "@alice:example.com"

    },

    "self_signing_key": {

      "keys": {

        "ed25519:base64+self+signing+public+key": "base64+self+signing+master+public+key"

      },

      "signatures": {

        "@alice:example.com": {

          "ed25519:base64+master+public+key": "signature+of+self+signing+key"

        }

      },

      "usage": [

        "self_signing"

      ],

      "user_id": "@alice:example.com"

    },

    "user_id": "@alice:example.com"

  },

  "edu_type": "m.signing_key_update"

}
```