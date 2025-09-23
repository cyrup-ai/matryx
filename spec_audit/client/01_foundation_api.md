---
title: "Matrix Client-Server API Foundation"
description: "Core Matrix client-server API foundations including standards, authentication, capabilities, filtering, events, and sync API basics"
---

# Matrix Client-Server API Foundation

You're looking at an unstable version of this specification. Unstable specifications may change at any time without notice.

[Switch to the current stable release](https://spec.matrix.org/latest).

The client-server API allows clients to send messages, control rooms and synchronise conversation history. It is designed to support both lightweight clients which store no state and lazy-load data from the server as required - as well as heavyweight clients which maintain a full local persistent copy of server state.

## API Standards

The mandatory baseline for client-server communication in Matrix is exchanging JSON objects over HTTP APIs. More efficient transports may be specified in future as optional extensions.

HTTPS is recommended for communication. The use of plain HTTP is not recommended outside test environments.

Clients are authenticated using opaque `access_token` strings (see [Client Authentication](#client-authentication) for details).

All `POST` and `PUT` endpoints, with the exception of those listed below, require the client to supply a request body containing a (potentially empty) JSON object. Clients should supply a `Content-Type` header of `application/json` for all requests with JSON bodies, but this is not required.

The exceptions are:

- [`POST /_matrix/media/v3/upload`](#post_matrixmediav3upload) and [`PUT /_matrix/media/v3/upload/{serverName}/{mediaId}`](#put_matrixmediav3uploadservernamemediaid), both of which take the uploaded media as the request body.
- [`POST /_matrix/client/v3/logout`](#post_matrixclientv3logout) and [`POST /_matrix/client/v3/logout/all`](#post_matrixclientv3logoutall), which take an empty request body.

Similarly, all endpoints require the server to return a JSON object, with the exception of 200 responses to the media download endpoints in the Content Repository module. Servers must include a `Content-Type` header of `application/json` for all JSON responses.

All JSON data, in requests or responses, must be encoded using UTF-8.

See also [Conventions for Matrix APIs](https://spec.matrix.org/unstable/appendices/#conventions-for-matrix-apis) in the Appendices for conventions which all Matrix APIs are expected to follow, and [Web Browser Clients](#web-browser-clients) below for additional requirements for server responses.

Any errors which occur at the Matrix API level MUST return a "standard error response". This is a JSON object which looks like:

```json
{
  "errcode": "<error code>",
  "error": "<error message>"
}
```

The `error` string will be a human-readable error message, usually a sentence explaining what went wrong.

The `errcode` string will be a unique string which can be used to handle an error message e.g. `M_FORBIDDEN`. Error codes should have their namespace first in ALL CAPS, followed by a single `_`. For example, if there was a custom namespace `com.mydomain.here`, and a `FORBIDDEN` code, the error code should look like `COM.MYDOMAIN.HERE_FORBIDDEN`. Error codes defined by this specification should start with `M_`.

Some `errcode` s define additional keys which should be present in the error response object, but the keys `error` and `errcode` MUST always be present.

Errors are generally best expressed by their error code rather than the HTTP status code returned. When encountering the error code `M_UNKNOWN`, clients should prefer the HTTP status code as a more reliable reference for what the issue was. For example, if the client receives an error code of `M_NOT_FOUND` but the request gave a 400 Bad Request status code, the client should treat the error as if the resource was not found. However, if the client were to receive an error code of `M_UNKNOWN` with a 400 Bad Request, the client should assume that the request being made was invalid.

These error codes can be returned by any API endpoint:

`M_FORBIDDEN` Forbidden access, e.g. joining a room without permission, failed login.

`M_UNKNOWN_TOKEN` The access or refresh token specified was not recognised.

An additional response parameter, `soft_logout`, might be present on the response for 401 HTTP status codes. See [the soft logout section](#soft-logout) for more information.

`M_MISSING_TOKEN` No access token was specified for the request.

`M_USER_LOCKED` The account has been [locked](#account-locking) and cannot be used at this time.

`M_USER_SUSPENDED` The account has been [suspended](#account-suspension) and can only be used for limited actions at this time.

`M_BAD_JSON` Request contained valid JSON, but it was malformed in some way, e.g. missing required keys, invalid values for keys.

`M_NOT_JSON` Request did not contain valid JSON.

`M_NOT_FOUND` No resource was found for this request.

`M_LIMIT_EXCEEDED` Too many requests have been sent in a short period of time. Wait a while then try again. See [Rate limiting](#rate-limiting).

`M_UNRECOGNIZED` The server did not understand the request. This is expected to be returned with a 404 HTTP status code if the endpoint is not implemented or a 405 HTTP status code if the endpoint is implemented, but the incorrect HTTP method is used.

`M_UNKNOWN` An unknown error has occurred.

The following error codes are specific to certain endpoints.

`M_UNAUTHORIZED` The request was not correctly authorized. Usually due to login failures.

`M_USER_DEACTIVATED` The user ID associated with the request has been deactivated. Typically for endpoints that prove authentication, such as [`/login`](#get_matrixclientv3login).

`M_USER_IN_USE` Encountered when trying to register a user ID which has been taken.

`M_INVALID_USERNAME` Encountered when trying to register a user ID which is not valid.

`M_ROOM_IN_USE` Sent when the room alias given to the `createRoom` API is already in use.

`M_INVALID_ROOM_STATE` Sent when the initial state given to the `createRoom` API is invalid.

`M_THREEPID_IN_USE` Sent when a threepid given to an API cannot be used because the same threepid is already in use.

`M_THREEPID_NOT_FOUND` Sent when a threepid given to an API cannot be used because no record matching the threepid was found.

`M_THREEPID_AUTH_FAILED` Authentication could not be performed on the third-party identifier.

`M_THREEPID_DENIED` The server does not permit this third-party identifier. This may happen if the server only permits, for example, email addresses from a particular domain.

`M_SERVER_NOT_TRUSTED` The client's request used a third-party server, e.g. identity server, that this server does not trust.

`M_UNSUPPORTED_ROOM_VERSION` The client's request to create a room used a room version that the server does not support.

`M_INCOMPATIBLE_ROOM_VERSION` The client attempted to join a room that has a version the server does not support. Inspect the `room_version` property of the error response for the room's version.

`M_BAD_STATE` The state change requested cannot be performed, such as attempting to unban a user who is not banned.

`M_GUEST_ACCESS_FORBIDDEN` The room or resource does not permit guests to access it.

`M_CAPTCHA_NEEDED` A Captcha is required to complete the request.

`M_CAPTCHA_INVALID` The Captcha provided did not match what was expected.

`M_MISSING_PARAM` A required parameter was missing from the request.

`M_INVALID_PARAM` A parameter that was specified has the wrong value. For example, the server expected an integer and instead received a string.

`M_TOO_LARGE` The request or entity was too large.

`M_EXCLUSIVE` The resource being requested is reserved by an application service, or the application service making the request has not created the resource.

`M_RESOURCE_LIMIT_EXCEEDED` The request cannot be completed because the homeserver has reached a resource limit imposed on it. For example, a homeserver held in a shared hosting environment may reach a resource limit if it starts using too much memory or disk space. The error MUST have an `admin_contact` field to provide the user receiving the error a place to reach out to. Typically, this error will appear on routes which attempt to modify state (e.g.: sending messages, account data, etc) and not routes which only read state (e.g.: [`/sync`](#get_matrixclientv3sync),[`/user/{userId}/account_data/{type}`](#get_matrixclientv3useruseridaccount_datatype), etc).

`M_CANNOT_LEAVE_SERVER_NOTICE_ROOM` The user is unable to reject an invite to join the server notices room. See the Server Notices module for more information.

`M_THREEPID_MEDIUM_NOT_SUPPORTED` The homeserver does not support adding a third party identifier of the given medium.

`M_THREEPID_IN_USE` The third party identifier specified by the client is not acceptable because it is already in use in some way.

### Rate limiting

Homeservers SHOULD implement rate limiting to reduce the risk of being overloaded. If a request is refused due to rate limiting, it should return a standard error response of the form:

```json
{
  "errcode": "M_LIMIT_EXCEEDED",
  "error": "string",
  "retry_after_ms": integer (optional, deprecated)
}
```

Homeservers SHOULD include a [`Retry-After`](https://www.rfc-editor.org/rfc/rfc9110#field.retry-after) header for any response with a 429 status code.

The `retry_after_ms` property MAY be included to tell the client how long they have to wait in milliseconds before they can try again. This property is deprecated, in favour of the `Retry-After` header.

**[Changed in `v1.10`]**: `retry_after_ms` property deprecated in favour of `Retry-After` header.

### Transaction identifiers

The client-server API typically uses `HTTP PUT` to submit requests with a client-generated transaction identifier in the HTTP path.

The purpose of the transaction ID is to allow the homeserver to distinguish a new request from a retransmission of a previous request so that it can make the request idempotent.

The transaction ID should **only** be used for this purpose.

After the request has finished, clients should change the `{txnId}` value for the next request. How this is achieved, is left as an implementation detail. It is recommended that clients use either version 4 UUIDs or a concatenation of the current timestamp and a monotonically increasing integer.

The homeserver should identify a request as a retransmission if the transaction ID is the same as a previous request, and the path of the HTTP request is the same.

Where a retransmission has been identified, the homeserver should return the same HTTP response code and content as the original request. For example, [`PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}`](#put_matrixclientv3roomsroomidsendeventtypetxnid) would return a `200 OK` with the `event_id` of the original request in the response body.

The scope of a transaction ID is for a single device, and a single HTTP endpoint. In other words: a single device could use the same transaction ID for a request to [`PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}`](#put_matrixclientv3roomsroomidsendeventtypetxnid) and [`PUT /_matrix/client/v3/sendToDevice/{eventType}/{txnId}`](#put_matrixclientv3sendtodeviceeventtypetxnid), and the two requests would be considered distinct because the two are considered separate endpoints. Similarly, if a client logs out and back in between two requests using the same transaction ID, the requests are distinct because the act of logging in and out creates a new device (unless an existing `device_id` is given during the [login](#login) process). On the other hand, if a client re-uses a transaction ID for the same endpoint after [refreshing](#refreshing-access-tokens) an access token, it will be assumed to be a duplicate request and ignored. See also [Relationship between access tokens and devices](#relationship-between-access-tokens-and-devices).

Some API endpoints may allow or require the use of `POST` requests without a transaction ID. Where this is optional, the use of a `PUT` request is strongly recommended.

## Web Browser Clients

It is realistic to expect that some clients will be written to be run within a web browser or similar environment. In these cases, the homeserver should respond to pre-flight requests and supply Cross-Origin Resource Sharing (CORS) headers on all requests.

Servers MUST expect that clients will approach them with `OPTIONS` requests, allowing clients to discover the CORS headers. All endpoints in this specification support the `OPTIONS` method, however the server MUST NOT perform any logic defined for the endpoints when approached with an `OPTIONS` request.

When a client approaches the server with a request, the server should respond with the CORS headers for that route. The recommended CORS headers to be returned by servers on all requests are:

```
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS
Access-Control-Allow-Headers: X-Requested-With, Content-Type, Authorization
```

## Server Discovery

In order to allow users to connect to a Matrix server without needing to explicitly specify the homeserver's URL or other parameters, clients SHOULD use an auto-discovery mechanism to determine the server's URL based on a user's Matrix ID. Auto-discovery should only be done at login time.

In this section, the following terms are used with specific meanings:

`PROMPT` Retrieve the specific piece of information from the user in a way which fits within the existing client user experience, if the client is inclined to do so. Failure can take place instead if no good user experience for this is possible at this point.

`IGNORE` Stop the current auto-discovery mechanism. If no more auto-discovery mechanisms are available, then the client may use other methods of determining the required parameters, such as prompting the user, or using default values.

`FAIL_PROMPT` Inform the user that auto-discovery failed due to invalid/empty data and `PROMPT` for the parameter.

`FAIL_ERROR` Inform the user that auto-discovery did not return any usable URLs. Do not continue further with the current login process. At this point, valid data was obtained, but no server is available to serve the client. No further guess should be attempted and the user should make a conscientious decision what to do next.

### Well-known URIs

Matrix facilitates automatic discovery for the Client-Server API base URL and more via the [RFC 8615](https://datatracker.ietf.org/doc/html/rfc8615) "Well-Known URI" method. This method uses JSON files at a predetermined location on the root path `/.well-known/` to specify parameter values.

The flow for auto-discovery is as follows:

1. Extract the [server name](https://spec.matrix.org/unstable/appendices/#server-name) from the user's Matrix ID by splitting the Matrix ID at the first colon.
2. Extract the hostname from the server name as described by the [grammar](https://spec.matrix.org/unstable/appendices/#server-name).
3. Make a GET request to `https://hostname/.well-known/matrix/client`.
	1. If the returned status code is 404, then `IGNORE`.
	2. If the returned status code is not 200, or the response body is empty, then `FAIL_PROMPT`.
	3. Parse the response body as a JSON object
		1. If the content cannot be parsed, then `FAIL_PROMPT`.
	4. Extract the `base_url` value from the `m.homeserver` property. This value is to be used as the base URL of the homeserver.
		1. If this value is not provided, then `FAIL_PROMPT`.
	5. Validate the homeserver base URL:
		1. Parse it as a URL. If it is not a URL, then `FAIL_ERROR`.
		2. Clients SHOULD validate that the URL points to a valid homeserver before accepting it by connecting to the [`/_matrix/client/versions`](#get_matrixclientversions) endpoint, ensuring that it does not return an error, and parsing and validating that the data conforms with the expected response format. If any step in the validation fails, then `FAIL_ERROR`. Validation is done as a simple check against configuration errors, in order to ensure that the discovered address points to a valid homeserver.
		3. It is important to note that the `base_url` value might include a trailing `/`. Consumers should be prepared to handle both cases.
	6. If the `m.identity_server` property is present, extract the `base_url` value for use as the base URL of the identity server. Validation for this URL is done as in the step above, but using `/_matrix/identity/v2` as the endpoint to connect to. If the `m.identity_server` property is present, but does not have a `base_url` value, then `FAIL_PROMPT`.
## GET /.well-known/matrix/support

---

**Added in `v1.10`**

Gets server admin contact and support page of the domain.

Note that this endpoint is not necessarily handled by the homeserver. It may be served by another webserver, used for discovering support information for the homeserver.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | No |

---

## Request

No request parameters or request body.

---

## Responses

| Status | Description |
| --- | --- |
| `200` | Server support information. |
| `404` | No server support information available. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `contacts` | `[[Contact](#getwell-knownmatrixsupport_response-200_contact)]` | Ways to contact the server administrator.  At least one of `contacts` or `support_page` is required. If only `contacts` is set, it must contain at least one item. |
| `support_page` | `[URI](https://datatracker.ietf.org/doc/html/rfc3986)` | The URL of a page to give users help specific to the homeserver, like extra login/registration steps.  At least one of `contacts` or `support_page` is required. |

| Name | Type | Description |
| --- | --- | --- |
| `email_address` | `[Email Address](https://datatracker.ietf.org/doc/html/rfc5321#section-4.1.2)` | An email address to reach the administrator.  At least one of `matrix_id` or `email_address` is required. |
| `matrix_id` | `[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers)` | A [Matrix User ID](https://spec.matrix.org/unstable/appendices/#user-identifiers) representing the administrator.  It could be an account registered on a different homeserver so the administrator can be contacted when the homeserver is down.  At least one of `matrix_id` or `email_address` is required. |
| `role` | `string` | **Required:** An informal description of what the contact methods are used for.  `m.role.admin` is a catch-all role for any queries and `m.role.security` is intended for sensitive requests.  Unspecified roles are permitted through the use of [Namespaced Identifiers](https://spec.matrix.org/unstable/appendices/#common-namespaced-identifier-grammar).  One of: `[m.role.admin, m.role.security]`. |

```json
{
  "contacts": [
    {
      "email_address": "admin@example.org",
      "matrix_id": "@admin:example.org",
      "role": "m.role.admin"
    },
    {
      "email_address": "security@example.org",
      "role": "m.role.security"
    }
  ],
  "support_page": "https://example.org/support.html"
}
```

### API Versions

Upon connecting, the Matrix client and server need to negotiate which version of the specification they commonly support, as the API evolves over time. The server advertises its supported versions and optionally unstable features to the client, which can then go on to make requests to the endpoints it supports.

