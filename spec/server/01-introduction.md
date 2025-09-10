# Introduction and API Standards

Matrix homeservers use the Federation APIs (also known as server-server APIs) to communicate with each other. Homeservers use these APIs to push messages to each other in real-time, to retrieve historic messages from each other, and to query profile and presence information about users on each other's servers.

The APIs are implemented using HTTPS requests between each of the servers. These HTTPS requests are strongly authenticated using public key signatures at the TLS transport layer and using public key signatures in HTTP Authorization headers at the HTTP layer.

## Types of Communication

There are three main kinds of communication that occur between homeservers:

### Persistent Data Units (PDUs)
These events are broadcast from one homeserver to any others that have joined the same room (identified by Room ID). They are persisted in long-term storage and record the history of messages and state for a room.

Like email, it is the responsibility of the originating server of a PDU to deliver that event to its recipient servers. However PDUs are signed using the originating server's private key so that it is possible to deliver them through third-party servers.

### Ephemeral Data Units (EDUs)
These events are pushed between pairs of homeservers. They are not persisted and are not part of the history of a room, nor does the receiving homeserver have to reply to them.

### Queries
These are single request/response interactions between a given pair of servers, initiated by one side sending an HTTPS GET request to obtain some information, and responded by the other. They are not persisted and contain no long-term significant history. They simply request a snapshot state at the instant the query is made.

EDUs and PDUs are further wrapped in an envelope called a Transaction, which is transferred from the origin to the destination homeserver using an HTTPS PUT request.

## API Standards

The mandatory baseline for server-server communication in Matrix is exchanging JSON objects over HTTPS APIs. More efficient transports may be specified in future as optional extensions.

All `POST` and `PUT` endpoints require the requesting server to supply a request body containing a (potentially empty) JSON object. Requesting servers should supply a `Content-Type` header of `application/json` for all requests with JSON bodies, but this is not required.

Similarly, all endpoints in this specification require the destination server to return a JSON object. Servers must include a `Content-Type` header of `application/json` for all JSON responses.

All JSON data, in requests or responses, must be encoded using UTF-8.

## TLS Requirements

### TLS Connection
Server-server communication must take place over HTTPS.

The destination server must provide a TLS certificate signed by a known Certificate Authority.

Requesting servers are ultimately responsible for determining the trusted Certificate Authorities, however are strongly encouraged to rely on the operating system's judgement. Servers can offer administrators a means to override the trusted authorities list. Servers can additionally skip the certificate validation for a given whitelist of domains or netmasks for the purposes of testing or in networks where verification is done elsewhere, such as with `.onion` addresses.

### SNI Support
Servers should respect SNI when making requests where possible: a SNI should be sent for the certificate which is expected, unless that certificate is expected to be an IP address in which case SNI is not supported and should not be sent.

### Certificate Transparency
Servers are encouraged to make use of the [Certificate Transparency](https://www.certificate-transparency.org/) project.

## Error Handling

### Unsupported Endpoints
If a request for an unsupported (or unknown) endpoint is received then the server must respond with a 404 `M_UNRECOGNIZED` error.

Similarly, a 405 `M_UNRECOGNIZED` error is used to denote an unsupported method to a known endpoint.

## Implementation Information

### GET /\_matrix/federation/v1/version

Get the implementation name and version of this homeserver.

| Rate-limited: | No |
| Requires authentication: | No |

#### Request
No request parameters or request body.

#### Response (200)
The implementation name and version of this homeserver.

**Response Format:**
```json
{
  "server": {
    "name": "My_Homeserver_Implementation", 
    "version": "ArbitraryVersionNumber"
  }
}
```

**Fields:**
- `server.name` (string, required): Arbitrary name that identify this implementation
- `server.version` (string, required): Version of this implementation. The version format depends on the implementation