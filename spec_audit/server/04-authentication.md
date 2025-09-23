# Authentication

## Request Authentication

Every HTTP request made by a homeserver is authenticated using public key digital signatures. The request method, target and body are signed by wrapping them in a JSON object and signing it using the JSON signing algorithm. The resulting signatures are added as an Authorization header with an auth scheme of `X-Matrix`. 

Note that the target field should include the full path starting with `/_matrix/...`, including the `?` and any query parameters if present, but should not include the leading `https:`, nor the destination server's hostname.

### Signing Process

**Step 1: Sign JSON**
```json
{
    "method": "POST",
    "uri": "/target",
    "origin": "origin.hs.example.com",
    "destination": "destination.hs.example.com",
    "content": <JSON-parsed request body>,
    "signatures": {
        "origin.hs.example.com": {
            "ed25519:key1": "ABCDEF..."
        }
    }
}
```

The server names in the JSON above are the server names for each homeserver involved. Delegation from the server name resolution section above do not affect these - the server names from before delegation would take place are used. This same condition applies throughout the request signing process.

**Step 2: Add Authorization Header**
```
POST /target HTTP/1.1
Authorization: X-Matrix origin="origin.hs.example.com",destination="destination.hs.example.com",key="ed25519:key1",sig="ABCDEF..."
Content-Type: application/json

<JSON-encoded request body>
```

### Authorization Header Format

The format of the Authorization header is given in Section 11.4 of RFC 9110. In summary, the header begins with authorization scheme `X-Matrix`, followed by one or more spaces, followed by a comma-separated list of parameters written as name=value pairs. Zero or more spaces and tabs around each comma are allowed. The names are case insensitive and order does not matter. The values must be enclosed in quotes if they contain characters that are not allowed in tokens, as defined in Section 5.6.2 of RFC 9110; if a value is a valid token, it may or may not be enclosed in quotes. Quoted values may include backslash-escaped characters. When parsing the header, the recipient must unescape the characters. That is, a backslash-character pair is replaced by the character that follows the backslash.

### Compatibility Guidelines

For compatibility with older servers, the sender should:
- only include one space after `X-Matrix`,
- only use lower-case names,
- avoid using backslashes in parameter values, and
- avoid including whitespace around the commas between name=value pairs.

For compatibility with older servers, the recipient should allow colons to be included in values without requiring the value to be enclosed in quotes.

### Authorization Parameters

The authorization parameters to include are:
- `origin`: the server name of the sending server. This is the same as the `origin` field from JSON described in step 1.
- `destination`: **[Added in v1.3]** the server name of the receiving server. This is the same as the `destination` field from the JSON described in step 1. For compatibility with older servers, recipients should accept requests without this parameter, but MUST always send it. If this property is included, but the value does not match the receiving server's name, the receiving server must deny the request with an HTTP status code 401 Unauthorized.
- `key`: the ID, including the algorithm name, of the sending server's key used to sign the request.
- `signature`: the signature of the JSON as calculated in step 1.

Unknown parameters are ignored.

## Response Authentication

Responses are authenticated by the TLS server certificate. A homeserver should not send a request until it has authenticated the connected server to avoid leaking messages to eavesdroppers.

## Client TLS Certificates

Requests are authenticated at the HTTP layer rather than at the TLS layer because HTTP services like Matrix are often deployed behind load balancers that handle the TLS and these load balancers make it difficult to check TLS client certificates.

A homeserver may provide a TLS client certificate and the receiving homeserver may check that the client certificate matches the certificate of the origin homeserver.