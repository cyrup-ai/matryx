# Server Keys and Cryptography

## Overview

Each homeserver publishes its public keys under `/_matrix/key/v2/server`. Homeservers query for keys by either getting `/_matrix/key/v2/server` directly or by querying an intermediate notary server using a `/_matrix/key/v2/query/{serverName}` API. Intermediate notary servers query the `/_matrix/key/v2/server` API on behalf of another server and sign the response with their own key. A server may query multiple notary servers to ensure that they all report the same public keys.

This approach is borrowed from the [Perspectives Project](https://web.archive.org/web/20170702024706/https://perspectives-project.org/), but modified to include the NACL keys and to use JSON instead of XML. It has the advantage of avoiding a single trust-root since each server is free to pick which notary servers they trust and can corroborate the keys returned by a given notary server by querying other servers.

## Publishing Keys

Homeservers publish their signing keys in a JSON object at `/_matrix/key/v2/server`. The response contains a list of `verify_keys` that are valid for signing federation requests made by the homeserver and for signing events. It contains a list of `old_verify_keys` which are only valid for signing events.

### GET /\_matrix/key/v2/server

Gets the homeserver's published signing keys. The homeserver may have any number of active keys and may have a number of old keys.

Intermediate notary servers should cache a response for half of its lifetime to avoid serving a stale response. Originating servers should avoid returning responses that expire in less than an hour to avoid repeated requests for a certificate that is about to expire. Requesting servers should limit how frequently they query for certificates to avoid flooding a server with requests.

If the server fails to respond to this request, intermediate notary servers should continue to return the last response they received from the server so that the signatures of old events can still be checked.

| Rate-limited: | No |
| Requires authentication: | No |

#### Request
No request parameters or request body.

#### Response (200)
The homeserver's keys

**Response Format:**
```json
{
  "old_verify_keys": {
    "ed25519:0ldk3y": {
      "expired_ts": 1532645052628,
      "key": "VGhpcyBzaG91bGQgYmUgeW91ciBvbGQga2V5J3MgZWQyNTUxOSBwYXlsb2FkLg"
    }
  },
  "server_name": "example.org",
  "signatures": {
    "example.org": {
      "ed25519:auto2": "VGhpcyBzaG91bGQgYWN0dWFsbHkgYmUgYSBzaWduYXR1cmU"
    }
  },
  "valid_until_ts": 1652262000000,
  "verify_keys": {
    "ed25519:abc123": {
      "key": "VGhpcyBzaG91bGQgYmUgYSByZWFsIGVkMjU1MTkgcGF5bG9hZA"
    }
  }
}
```

**Fields:**
- `server_name` (string, required): The homeserver's server name
- `verify_keys` (object, required): Public keys of the homeserver for verifying digital signatures. The object's key is the algorithm and version combined (e.g., `ed25519:abc123`). Together, this forms the Key ID. The version must have characters matching the regular expression `[a-zA-Z0-9_]`
- `old_verify_keys` (object): The public keys that the server used to use and when it stopped using them. Same key format as `verify_keys`
- `valid_until_ts` (integer, required): POSIX timestamp in milliseconds when the list of valid keys should be refreshed. This field MUST be ignored in room versions 1, 2, 3, and 4. Keys used beyond this timestamp MUST be considered invalid, depending on the room version specification. Servers MUST use the lesser of this field and 7 days into the future when determining if a key is valid
- `signatures` (object, required): Digital signatures for this object signed using the `verify_keys`. The signature is calculated using the process described at Signing JSON

**Verify Key Object:**
- `key` (string, required): The Unpadded base64 encoded key

**Old Verify Key Object:**  
- `key` (string, required): The Unpadded base64 encoded key
- `expired_ts` (integer, required): POSIX timestamp in milliseconds for when this key expired

## Querying Keys Through Notary Servers

Servers may query another server's keys through a notary server. The notary server may be another homeserver. The notary server will retrieve keys from the queried servers through use of the `/_matrix/key/v2/server` API. The notary server will additionally sign the response from the queried server before returning the results.

Notary servers can return keys for servers that are offline or having issues serving their own keys by using cached responses. Keys can be queried from multiple servers to mitigate against DNS spoofing.

### POST /\_matrix/key/v2/query

Query for keys from multiple servers in a batch format. The receiving (notary) server must sign the keys returned by the queried servers.

| Rate-limited: | No |
| Requires authentication: | No |

#### Request Body
```json
{
  "server_keys": {
    "example.org": {
      "ed25519:abc123": {
        "minimum_valid_until_ts": 1234567890
      }
    }
  }
}
```

**Fields:**
- `server_keys` (object, required): The query criteria. The outer string key on the object is the server name (e.g., `matrix.org`). The inner string key is the Key ID to query for the particular server. If no key IDs are given to be queried, the notary server should query for all keys. If no servers are given, the notary server must return an empty `server_keys` array in the response. The notary server may return multiple keys regardless of the Key IDs given

**Query Criteria:**
- `minimum_valid_until_ts` (integer): A millisecond POSIX timestamp in milliseconds indicating when the returned certificates will need to be valid until to be useful to the requesting server. If not supplied, the current time as determined by the notary server is used

#### Response (200)
The keys for the queried servers, signed by the notary server. Servers which are offline and have no cached keys will not be included in the result. This may result in an empty array.

**Response Format:**
```json
{
  "server_keys": [
    {
      "old_verify_keys": {
        "ed25519:0ldK3y": {
          "expired_ts": 1532645052628,
          "key": "VGhpcyBzaG91bGQgYmUgeW91ciBvbGQga2V5J3MgZWQyNTUxOSBwYXlsb2FkLg"
        }
      },
      "server_name": "example.org",
      "signatures": {
        "example.org": {
          "ed25519:abc123": "VGhpcyBzaG91bGQgYWN0dWFsbHkgYmUgYSBzaWduYXR1cmU"
        },
        "notary.server.com": {
          "ed25519:010203": "VGhpcyBpcyBhbm90aGVyIHNpZ25hdHVyZQ"
        }
      },
      "valid_until_ts": 1652262000000,
      "verify_keys": {
        "ed25519:abc123": {
          "key": "VGhpcyBzaG91bGQgYmUgYSByZWFsIGVkMjU1MTkgcGF5bG9hZA"
        }
      }
    }
  ]
}
```

### GET /\_matrix/key/v2/query/{serverName}

Query for another server's keys. The receiving (notary) server must sign the keys returned by the queried server.

| Rate-limited: | No |
| Requires authentication: | No |

#### Request Parameters
- `serverName` (string, required): The server name to query
- `minimum_valid_until_ts` (integer): A millisecond POSIX timestamp indicating when the returned certificates will need to be valid until to be useful to the requesting server. If not supplied, the current time as determined by the notary server is used

#### Response (200)
The keys for the server, or an empty array if the server could not be reached and no cached keys were available. Same format as batch query response.