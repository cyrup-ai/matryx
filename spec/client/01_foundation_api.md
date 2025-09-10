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
## GET /.well-known/matrix/client

---

Gets discovery information about the domain. The file may include additional keys, which MUST follow the Java package naming convention, e.g. `com.example.myapp.property`. This ensures property names are suitably namespaced for each application and reduces the risk of clashes.

Note that this endpoint is not necessarily handled by the homeserver, but by another webserver, to be used for discovering the homeserver URL.

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
| `200` | Server discovery information. |
| `404` | No server discovery information available. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `m.homeserver` | `[Homeserver Information](#getwell-knownmatrixclient_response-200_homeserver-information)` | **Required:** Used by clients to discover homeserver information. |
| `m.identity_server` | `[Identity Server Information](#getwell-knownmatrixclient_response-200_identity-server-information)` | Used by clients to discover identity server information. |
| <Other properties> |  | Application-dependent keys using Java package naming convention. |

| Name | Type | Description |
| --- | --- | --- |
| `base_url` | `[URI](https://datatracker.ietf.org/doc/html/rfc3986)` | **Required:** The base URL for the homeserver for client-server connections. |

| Name | Type | Description |
| --- | --- | --- |
| `base_url` | `[URI](https://datatracker.ietf.org/doc/html/rfc3986)` | **Required:** The base URL for the identity server for client-server connections. |

```json
{
  "m.homeserver": {
    "base_url": "https://matrix.example.com"
  },
  "m.identity_server": {
    "base_url": "https://identity.example.com"
  },
  "org.example.custom.property": {
    "app_url": "https://custom.app.example.org"
  }
}
```

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

## GET /\_matrix/client/versions

---

**Changed in `v1.10`:** This endpoint can behave differently when authentication is provided.

Gets the versions of the specification supported by the server.

Values will take the form `vX.Y` or `rX.Y.Z` in historical cases. See [the Specification Versioning](https://spec.matrix.org/unstable/#specification-versions) for more information.

The server may additionally advertise experimental features it supports through `unstable_features`. These features should be namespaced and may optionally include version information within their name if desired. Features listed here are not for optionally toggling parts of the Matrix specification and should only be used to advertise support for a feature which has not yet landed in the spec. For example, a feature currently undergoing the proposal process may appear here and eventually be taken off this list once the feature lands in the spec and the server deems it reasonable to do so. Servers can choose to enable some features only for some users, so clients should include authentication in the request to get all the features available for the logged-in user. If no authentication is provided, the server should only return the features available to all users. Servers may wish to keep advertising features here after they've been released into the spec to give clients a chance to upgrade appropriately. Additionally, clients should avoid using unstable features in their stable releases.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Optional |

---

## Request

No request parameters or request body.

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The versions supported by the server. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `unstable_features` | `{string: boolean}` | Experimental features the server supports. Features not listed here, or the lack of this property all together, indicate that a feature is not supported. |
| `versions` | `[string]` | **Required:** The supported versions. |

```json
{
  "unstable_features": {
    "org.example.my_feature": true
  },
  "versions": [
    "r0.0.1",
    "v1.1"
  ]
}
```

## Client Authentication

**[Changed in `v1.15`]** OAuth 2.0 API added to the specification.

Most API endpoints require the user to identify themselves by presenting previously obtained credentials in the form of an access token. An access token is typically obtained via the [Login](#login) or [Registration](#account-registration) processes. Access tokens can expire; a new access token can be generated by using a refresh token.

Since Matrix 1.15, the Client-Server specification supports two authentication APIs:

- The [legacy API](#legacy-api)
- The [OAuth 2.0 API](#oauth-20-api)

The legacy API has existed since the first version of the Matrix specification, while the OAuth 2.0 API has been introduced to rely on a industry standard and its experience rather than implementing a custom protocol that might not follow the best practices.

A homeserver may support one of those two APIs, or both. The two APIs are mutually incompatible, which means that after logging in, clients MUST only use the API that was used to obtain their current access token.

### Authentication API discovery

To discover if a homeserver supports the legacy API, the [`GET /login`](#get_matrixclientv3login) endpoint can be used.

To discover if a homeserver supports the OAuth 2.0 API, the [`GET /auth_metadata`](#get_matrixclientv1auth_metadata) endpoint can be used.

In both cases, the server SHOULD respond with 404 and an `M_UNRECOGNIZED` error code if the corresponding API is not supported.

### Account registration

With the legacy API, a client can register a new account with the [`/register`](#post_matrixclientv3register) endpoint.

With the OAuth 2.0 API, a client can't register a new account directly. The end user must do that directly in the homeserver's web UI. However, the client can signal to the homeserver that the user wishes to create a new account with the [`prompt=create`](#user-registration) parameter during authorization.

### Login

With the legacy API, a client can obtain an access token by using one of the [login](#legacy-login) methods supported by the homeserver at the [`POST /login`](#post_matrixclientv3login) endpoint. To invalidate the access token the client must call the [`/logout`](#post_matrixclientv3logout) endpoint.

With the OAuth 2.0 API, a client can obtain an access token by using one of the [grant types](#grant-types) supported by the homeserver and authorizing the proper [scope](#scope), as demonstrated in the [login flow](#login-flow). To invalidate the access token the client must use [token revocation](#token-revocation).

### Using access tokens

Access tokens may be provided via a request header, using the Authentication Bearer scheme: `Authorization: Bearer TheTokenHere`.

Clients may alternatively provide the access token via a query string parameter:`access_token=TheTokenHere`. This method is deprecated to prevent the access token being leaked in access/HTTP logs and SHOULD NOT be used by clients.

Homeservers MUST support both methods.

When credentials are required but missing or invalid, the HTTP call will return with a status of 401 and the error code, `M_MISSING_TOKEN` or `M_UNKNOWN_TOKEN` respectively. Note that an error code of `M_UNKNOWN_TOKEN` could mean one of four things:

1. the access token was never valid.
2. the access token has been logged out.
3. the access token has been [soft logged out](#soft-logout).
4. **[Added in `v1.3`]** the access token [needs to be refreshed](#refreshing-access-tokens).

When a client receives an error code of `M_UNKNOWN_TOKEN`, it should:

- attempt to [refresh the token](#refreshing-access-tokens), if it has a refresh token;
- if [`soft_logout`](#soft-logout) is set to `true`, it can offer to re-log in the user, retaining any of the client's persisted information;
- otherwise, consider the user as having been logged out.

### Relationship between access tokens and devices

Client [devices](https://spec.matrix.org/unstable/#devices) are closely related to access tokens and refresh tokens. Matrix servers should record which device each access token and refresh token are assigned to, so that subsequent requests can be handled correctly. When a refresh token is used to generate a new access token and refresh token, the new access and refresh tokens are now bound to the device associated with the initial refresh token.

During login or registration, the generated access token should be associated with a `device_id`. The legacy [Login](#legacy-login) and [Registration](#legacy-account-registration) processes auto-generate a new `device_id`, but a client is also free to provide its own `device_id`. With the OAuth 2.0 API, the `device_id` is always provided by the client. The client can generate a new `device_id` or, provided the user remains the same, reuse an existing device. If the client sets the `device_id`, the server will invalidate any access and refresh tokens previously assigned to that device.

### Refreshing access tokens

**[Added in `v1.3`]**

Access tokens can expire after a certain amount of time. Any HTTP calls that use an expired access token will return with an error code `M_UNKNOWN_TOKEN`, preferably with `soft_logout: true`. When a client receives this error and it has a refresh token, it should attempt to refresh the access token. Clients can also refresh their access token at any time, even if it has not yet expired. If the token refresh succeeds, the client should use the new token for future requests, and can re-try previously-failed requests with the new token. When an access token is refreshed, a new refresh token may be returned; if a new refresh token is given, the old refresh token will be invalidated, and the new refresh token should be used when the access token needs to be refreshed.

The old refresh token remains valid until the new access token or refresh token is used, at which point the old refresh token is revoked. This ensures that if a client fails to receive or persist the new tokens, it will be able to repeat the refresh operation.

If the token refresh fails and the error response included a `soft_logout: true` property, then the client can treat it as a [soft logout](#soft-logout) and attempt to obtain a new access token by re-logging in. If the error response does not include a `soft_logout: true` property, the client should consider the user as being logged out.

With the legacy API, refreshing access tokens is done by calling [`/refresh`](#post_matrixclientv3refresh). Handling of clients that do not support refresh tokens is up to the homeserver; clients indicate their support for refresh tokens by including a `refresh_token: true` property in the request body of the [`/login`](#post_matrixclientv3login) and [`/register`](#post_matrixclientv3register) endpoints. For example, homeservers may allow the use of non-expiring access tokens, or may expire access tokens anyways and rely on soft logout behaviour on clients that don't support refreshing.

With the OAuth 2.0 API, refreshing access tokens is done with the [refresh token grant type](#refresh-token-grant), as demonstrated in the [token refresh flow](#token-refresh-flow). Support for refreshing access tokens is mandatory with this API.

### Soft logout

A client can be in a "soft logout" state if the server requires re-authentication before continuing, but does not want to invalidate the client's session. The server indicates that the client is in a soft logout state by including a `soft_logout: true` parameter in an `M_UNKNOWN_TOKEN` error response; the `soft_logout` parameter defaults to `false`. If the `soft_logout` parameter is omitted or is `false`, this means the server has destroyed the session and the client should not reuse it. That is, any persisted state held by the client, such as encryption keys and device information, must not be reused and must be discarded. If `soft_logout` is `true` the client can reuse any persisted state.

**[Changed in `v1.3`]** A client that receives such a response can try to [refresh its access token](#refreshing-access-tokens), if it has a refresh token available. If it does not have a refresh token available, or refreshing fails with `soft_logout: true`, the client can acquire a new access token by specifying the device ID it is already using to the login API.

**[Changed in `v1.12`]** A client that receives such a response together with an `M_USER_LOCKED` error code, cannot obtain a new access token until the account has been [unlocked](#account-locking).

### Account management

With the legacy API, a client can use several endpoints to allow the user to manage their account like [changing their password](#password-management),[managing their devices](#device-management) or [deactivating their account](#account-deactivation).

With the OAuth 2.0 API, all account management is done via the homeserver's web UI.### Legacy API

This is the original authentication API that was introduced in the first version of the Client-Server specification and uses custom APIs. Contrary to the OAuth 2.0 API, account management is primarily done in the client's interface and as such it does not usually require the end user to be redirected to a web UI in their browser.

#### User-Interactive Authentication API

##### Overview

Some API endpoints require authentication that interacts with the user. The homeserver may provide many different ways of authenticating, such as user/password auth, login via a single-sign-on server (SSO), etc. This specification does not define how homeservers should authorise their users but instead defines the standard interface which implementations should follow so that ANY client can log in to ANY homeserver.

The process takes the form of one or more 'stages'. At each stage the client submits a set of data for a given authentication type and awaits a response from the server, which will either be a final success or a request to perform an additional stage. This exchange continues until the final success.

For each endpoint, a server offers one or more 'flows' that the client can use to authenticate itself. Each flow comprises a series of stages, as described above. The client is free to choose which flow it follows, however the flow's stages must be completed in order. Failing to follow the flows in order must result in an HTTP 401 response, as defined below. When all stages in a flow are complete, authentication is complete and the API call succeeds.

##### User-interactive API in the REST API

In the REST API described in this specification, authentication works by the client and server exchanging JSON dictionaries. The server indicates what authentication data it requires via the body of an HTTP 401 response, and the client submits that authentication data via the `auth` request parameter.

A client should first make a request with no `auth` parameter. The homeserver returns an HTTP 401 response, with a JSON body, as follows:

```
HTTP/1.1 401 Unauthorized
Content-Type: application/json
```
```json
{
  "flows": [
    {
      "stages": [ "example.type.foo", "example.type.bar" ]
    },
    {
      "stages": [ "example.type.foo", "example.type.baz" ]
    }
  ],
  "params": {
      "example.type.baz": {
          "example_key": "foobar"
      }
  },
  "session": "xxxxxx"
}
```

In addition to the `flows`, this object contains some extra information:

- `params`: This section contains any information that the client will need to know in order to use a given type of authentication. For each authentication type presented, that type may be present as a key in this dictionary. For example, the public part of an OAuth client ID could be given here.
- `session`: This is a session identifier that the client must pass back to the homeserver, if one is provided, in subsequent attempts to authenticate in the same API call.

The client then chooses a flow and attempts to complete the first stage. It does this by resubmitting the same request with the addition of an `auth` key in the object that it submits. This dictionary contains a `type` key whose value is the name of the authentication type that the client is attempting to complete. It must also contain a `session` key with the value of the session key given by the homeserver, if one was given. It also contains other keys dependent on the auth type being attempted. For example, if the client is attempting to complete auth type `example.type.foo`, it might submit something like this:

```
POST /_matrix/client/v3/endpoint HTTP/1.1
Content-Type: application/json
```

If the homeserver deems the authentication attempt to be successful but still requires more stages to be completed, it returns HTTP status 401 along with the same object as when no authentication was attempted, with the addition of the `completed` key which is an array of auth types the client has completed successfully:

```
HTTP/1.1 401 Unauthorized
Content-Type: application/json
```
```json
{
  "completed": [ "example.type.foo" ],
  "flows": [
    {
      "stages": [ "example.type.foo", "example.type.bar" ]
    },
    {
      "stages": [ "example.type.foo", "example.type.baz" ]
    }
  ],
  "params": {
      "example.type.baz": {
          "example_key": "foobar"
      }
  },
  "session": "xxxxxx"
}
```

Individual stages may require more than one request to complete, in which case the response will be as if the request was unauthenticated with the addition of any other keys as defined by the auth type.

If the homeserver decides that an attempt on a stage was unsuccessful, but the client may make a second attempt, it returns the same HTTP status 401 response as above, with the addition of the standard `errcode` and `error` fields describing the error. For example:

```
HTTP/1.1 401 Unauthorized
Content-Type: application/json
```
```json
{
  "errcode": "M_FORBIDDEN",
  "error": "Invalid password",
  "completed": [ "example.type.foo" ],
  "flows": [
    {
      "stages": [ "example.type.foo", "example.type.bar" ]
    },
    {
      "stages": [ "example.type.foo", "example.type.baz" ]
    }
  ],
  "params": {
      "example.type.baz": {
          "example_key": "foobar"
      }
  },
  "session": "xxxxxx"
}
```

If the request fails for a reason other than authentication, the server returns an error message in the standard format. For example:

```
HTTP/1.1 400 Bad request
Content-Type: application/json
```
```json
{
  "errcode": "M_EXAMPLE_ERROR",
  "error": "Something was wrong"
}
```

If the client has completed all stages of a flow, the homeserver performs the API call and returns the result as normal. Completed stages cannot be retried by clients, therefore servers must return either a 401 response with the completed stages, or the result of the API call if all stages were completed when a client retries a stage.

Some authentication types may be completed by means other than through the Matrix client, for example, an email confirmation may be completed when the user clicks on the link in the email. In this case, the client retries the request with an auth dict containing only the session key. The response to this will be the same as if the client were attempting to complete an auth state normally, i.e. the request will either complete or request auth, with the presence or absence of that auth type in the 'completed' array indicating whether that stage is complete.

##### Fallback

Clients cannot be expected to be able to know how to process every single login type. If a client does not know how to handle a given login type, it can direct the user to a web browser with the URL of a fallback page which will allow the user to complete that login step out-of-band in their web browser. The URL it should open is:

```
/_matrix/client/v3/auth/<auth type>/fallback/web?session=<session ID>
```

Where `auth type` is the type name of the stage it is attempting and `session ID` is the ID of the session given by the homeserver.

This MUST return an HTML page which can perform this authentication stage. This page must use the following JavaScript when the authentication has been completed:

```js
if (window.onAuthDone) {
    window.onAuthDone();
} else if (window.opener && window.opener.postMessage) {
    window.opener.postMessage("authDone", "*");
}
```

This allows the client to either arrange for the global function `onAuthDone` to be defined in an embedded browser, or to use the HTML5 [cross-document messaging](https://www.w3.org/TR/webmessaging/#web-messaging) API, to receive a notification that the authentication stage has been completed.

Once a client receives the notification that the authentication stage has been completed, it should resubmit the request with an auth dict with just the session ID:

```json
{
  "session": "<session ID>"
}
```## Capabilities Negotiation

A homeserver may not support certain operations and clients must be able to query for what the homeserver can and can't offer. For example, a homeserver may not support users changing their password as it is configured to perform authentication against an external system.

The capabilities advertised through this system are intended to advertise functionality which is optional in the API, or which depend in some way on the state of the user or server. This system should not be used to advertise unstable or experimental features - this is better done by the [`/versions`](#get_matrixclientversions) endpoint.

Some examples of what a reasonable capability could be are:

- Whether the server supports user presence.
- Whether the server supports optional features, such as the user or room directories.
- The rate limits or file type restrictions imposed on clients by the server.

Some examples of what should **not** be a capability are:

- Whether the server supports a feature in the `unstable` specification.
- Media size limits - these are handled by the [`/config`](#get_matrixmediav3config) API.
- Optional encodings or alternative transports for communicating with the server.

Capabilities prefixed with `m.` are reserved for definition in the Matrix specification while other values may be used by servers using the Java package naming convention. The capabilities supported by the Matrix specification are defined later in this section.

## GET /_matrix/client/v3/capabilities

---

Gets information about the server's supported feature set and other relevant capabilities.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

No request parameters or request body.

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The capabilities of the server. |
| `429` | This request was rate-limited. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `capabilities` | `[Capabilities](#get_matrixclientv3capabilities_response-200_capabilities)` | **Required:** The custom capabilities the server supports, using the Java package naming convention. |

| Name | Type | Description |
| --- | --- | --- |
| `m.3pid_changes` | `[BooleanCapability](#get_matrixclientv3capabilities_response-200_booleancapability)` | Capability to indicate if the user can change 3PID associations on their account. |
| `m.change_password` | `[BooleanCapability](#get_matrixclientv3capabilities_response-200_booleancapability)` | Capability to indicate if the user can change their password. |
| `m.get_login_token` | `[BooleanCapability](#get_matrixclientv3capabilities_response-200_booleancapability)` | Capability to indicate if the user can generate tokens to log further clients into their account. |
| `m.room_versions` | `[RoomVersionsCapability](#get_matrixclientv3capabilities_response-200_roomversionscapability)` | The room versions the server supports. |
| `m.set_avatar_url` | `[BooleanCapability](#get_matrixclientv3capabilities_response-200_booleancapability)` | **Deprecated:** Capability to indicate if the user can change their avatar. |
| `m.set_displayname` | `[BooleanCapability](#get_matrixclientv3capabilities_response-200_booleancapability)` | **Deprecated:** Capability to indicate if the user can change their display name. |
| <Other properties> | | Application-dependent keys using the Common Namespaced Identifier Grammar. |

| Name | Type | Description |
| --- | --- | --- |
| `enabled` | `boolean` | **Required:** True if the user can perform the action, false otherwise. |

| Name | Type | Description |
| --- | --- | --- |
| `available` | `{string: string}` | **Required:** A detailed description of the room versions the server supports. |
| `default` | `string` | **Required:** The default room version the server is using for new rooms. |

```json
{
  "capabilities": {
    "com.example.custom.ratelimit": {
      "max_requests_per_hour": 600
    },
    "m.change_password": {
      "enabled": false
    },
    "m.room_versions": {
      "available": {
        "1": "stable",
        "2": "stable",
        "3": "unstable",
        "test-version": "unstable"
      },
      "default": "1"
    }
  }
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

## Filtering

Filters can be created on the server and can be passed as a parameter to APIs which return events. These filters alter the data returned from those APIs. Not all APIs accept filters.

Membership events often take significant resources for clients to track. In an effort to reduce the number of resources used, clients can enable "lazy-loading" for room members. By doing this, servers will attempt to only send membership events which are relevant to the client.

It is important to understand that lazy-loading is not intended to be a perfect optimisation, and that it may not be practical for the server to calculate precisely which membership events are relevant to the client. As a result, it is valid for the server to send redundant membership events to the client to ease implementation, although such redundancy should be minimised where possible to conserve bandwidth.

In terms of filters, lazy-loading is enabled by enabling `lazy_load_members` on a [`RoomEventFilter`](#post_matrixclientv3useruseridfilter_request_roomeventfilter). When enabled, lazy-loading aware endpoints (see below) will only include membership events for the `sender` of events being included in the response. For example, if a client makes a `/sync` request with lazy-loading enabled, the server will only return membership events for the `sender` of events in the timeline, not all members of a room.

When processing a sequence of events (e.g. by looping on [`/sync`](#get_matrixclientv3sync) or paginating [`/messages`](#get_matrixclientv3roomsroomidmessages)), it is common for blocks of events in the sequence to share a similar set of senders. Rather than responses in the sequence sending duplicate membership events for these senders to the client, the server MAY assume that clients will remember membership events they have already been sent, and choose to skip sending membership events for members whose membership has not changed. These are called 'redundant membership events'. Clients may request that redundant membership events are always included in responses by setting `include_redundant_members` to true in the filter.

The expected pattern for using lazy-loading is currently:

- Client performs an initial /sync with lazy-loading enabled, and receives only the membership events which relate to the senders of the events it receives.
- Clients which support display-name tab-completion or other operations which require rapid access to all members in a room should call /members for the currently selected room, with an `?at` parameter set to the /sync response's from token. The member list for the room is then maintained by the state in subsequent incremental /sync responses.
- Clients which do not support tab-completion may instead pull in profiles for arbitrary users (e.g. read receipts, typing notifications) on demand by querying the room state or [`/profile`](#get_matrixclientv3profileuserid).

The current endpoints which support lazy-loading room members are:

- [`/sync`](#get_matrixclientv3sync)
- [`/rooms/<room_id>/messages`](#get_matrixclientv3roomsroomidmessages)
- [`/rooms/{roomId}/context/{eventId}`](#get_matrixclientv3roomsroomidcontexteventid)

### API endpoints

## POST /_matrix/client/v3/user/{userId}/filter

---

Uploads a new filter definition to the homeserver. Returns a filter ID that may be used in future requests to restrict which events are returned to the client.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `userId` | `string` | **Required:** The id of the user uploading the filter. The access token must be authorized to make requests for this user id. |

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `account_data` | `[EventFilter](#post_matrixclientv3useruseridfilter_request_eventfilter)` | The user account data that isn't associated with rooms to include. |
| `event_fields` | `[string]` | List of event fields to include. If this list is absent then all fields are included. The entries are dot-separated paths for each property to include. So ['content.body'] will include the 'body' field of the 'content' object. A server may include more fields than were requested. |
| `event_format` | `string` | The format to use for events. 'client' will return the events in a format suitable for clients. 'federation' will return the raw event as received over federation. The default is 'client'. One of: `[client, federation]`. |
| `presence` | `[EventFilter](#post_matrixclientv3useruseridfilter_request_eventfilter)` | The presence updates to include. |
| `room` | `[RoomFilter](#post_matrixclientv3useruseridfilter_request_roomfilter)` | Filters to be applied to room data. |

### Request body example

```json
{
  "event_fields": [
    "type",
    "content",
    "sender"
  ],
  "event_format": "client",
  "presence": {
    "not_senders": [
      "@alice:example.com"
    ],
    "types": [
      "m.presence"
    ]
  },
  "room": {
    "ephemeral": {
      "not_rooms": [
        "!726s6s6q:example.com"
      ],
      "not_senders": [
        "@spam:example.com"
      ],
      "types": [
        "m.receipt",
        "m.typing"
      ]
    },
    "state": {
      "not_rooms": [
        "!726s6s6q:example.com"
      ],
      "types": [
        "m.room.*"
      ]
    },
    "timeline": {
      "limit": 10,
      "not_rooms": [
        "!726s6s6q:example.com"
      ],
      "not_senders": [
        "@spam:example.com"
      ],
      "types": [
        "m.room.message"
      ]
    }
  }
}
```

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The filter was created. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `filter_id` | `string` | **Required:** The ID of the filter that was created. Cannot start with a `{` as this character is used to determine if the filter provided to endpoints is a filter ID or a filter definition. |

```json
{
  "filter_id": "66696p746572"
}
```

## GET /_matrix/client/v3/user/{userId}/filter/{filterId}

---

Download a filter

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `filterId` | `string` | **Required:** The filter ID to download. |
| `userId` | `string` | **Required:** The user ID to download a filter for. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The filter definition. |
| `404` | Unknown filter. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `account_data` | `[EventFilter](#get_matrixclientv3useruseridfilterfilterid_response-200_eventfilter)` | The user account data that isn't associated with rooms to include. |
| `event_fields` | `[string]` | List of event fields to include. If this list is absent then all fields are included. The entries are dot-separated paths for each property to include. So ['content.body'] will include the 'body' field of the 'content' object. A server may include more fields than were requested. |
| `event_format` | `string` | The format to use for events. 'client' will return the events in a format suitable for clients. 'federation' will return the raw event as received over federation. The default is 'client'. One of: `[client, federation]`. |
| `presence` | `[EventFilter](#get_matrixclientv3useruseridfilterfilterid_response-200_eventfilter)` | The presence updates to include. |
| `room` | `[RoomFilter](#get_matrixclientv3useruseridfilterfilterid_response-200_roomfilter)` | Filters to be applied to room data. |

```json
{
  "event_fields": [
    "type",
    "content",
    "sender"
  ],
  "event_format": "client",
  "presence": {
    "not_senders": [
      "@alice:example.com"
    ],
    "types": [
      "m.presence"
    ]
  },
  "room": {
    "ephemeral": {
      "not_rooms": [
        "!726s6s6q:example.com"
      ],
      "not_senders": [
        "@spam:example.com"
      ],
      "types": [
        "m.receipt",
        "m.typing"
      ]
    },
    "state": {
      "not_rooms": [
        "!726s6s6q:example.com"
      ],
      "types": [
        "m.room.*"
      ]
    },
    "timeline": {
      "limit": 10,
      "not_rooms": [
        "!726s6s6q:example.com"
      ],
      "not_senders": [
        "@spam:example.com"
      ],
      "types": [
        "m.room.message"
      ]
    }
  }
}
```
## GET /_matrix/client/v3/user/{userId}/filter/{filterId}

---

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `filterId` | `string` | **Required:** The filter ID to download. |
| `userId` | `string` | **Required:** The user ID to download a filter for. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The filter definition. |
| `404` | Unknown filter. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `account_data` | `[EventFilter](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3useruseridfilterfilterid_response-200_eventfilter)` | The user account data that isn't associated with rooms to include. |
| `event_fields` | `[string]` | List of event fields to include. If this list is absent then all fields are included. The entries are [dot-separated paths for each property](https://spec.matrix.org/unstable/appendices/#dot-separated-property-paths) to include. So \['content.body'\] will include the 'body' field of the 'content' object. A server may include more fields than were requested. |
| `event_format` | `string` | The format to use for events. 'client' will return the events in a format suitable for clients. 'federation' will return the raw event as received over federation. The default is 'client'.  One of: `[client, federation]`. |
| `presence` | `[EventFilter](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3useruseridfilterfilterid_response-200_eventfilter)` | The presence updates to include. |
| `room` | `[RoomFilter](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3useruseridfilterfilterid_response-200_roomfilter)` | Filters to be applied to room data. |

#### EventFilter

| Name | Type | Description |
| --- | --- | --- |
| `limit` | `integer` | The maximum number of events to return, must be an integer greater than 0.  Servers should apply a default value, and impose a maximum value to avoid resource exhaustion. |
| `not_senders` | `[string]` | A list of sender IDs to exclude. If this list is absent then no senders are excluded. A matching sender will be excluded even if it is listed in the `'senders'` filter. |
| `not_types` | `[string]` | A list of event types to exclude. If this list is absent then no event types are excluded. A matching type will be excluded even if it is listed in the `'types'` filter. A '\*' can be used as a wildcard to match any sequence of characters. |
| `senders` | `[string]` | A list of senders IDs to include. If this list is absent then all senders are included. |
| `types` | `[string]` | A list of event types to include. If this list is absent then all event types are included. A `'*'` can be used as a wildcard to match any sequence of characters. |

#### RoomFilter

| Name | Type | Description |
| --- | --- | --- |
| `account_data` | `[RoomEventFilter](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3useruseridfilterfilterid_response-200_roomeventfilter)` | The per user account data to include for rooms. |
| `ephemeral` | `[RoomEventFilter](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3useruseridfilterfilterid_response-200_roomeventfilter)` | The ephemeral events to include for rooms. These are the events that appear in the `ephemeral` property in the `/sync` response. |
| `include_leave` | `boolean` | Include rooms that the user has left in the sync, default false |
| `not_rooms` | `[string]` | A list of room IDs to exclude. If this list is absent then no rooms are excluded. A matching room will be excluded even if it is listed in the `'rooms'` filter. This filter is applied before the filters in `ephemeral`, `state`, `timeline` or `account_data` |
| `rooms` | `[string]` | A list of room IDs to include. If this list is absent then all rooms are included. This filter is applied before the filters in `ephemeral`, `state`, `timeline` or `account_data` |
| `state` | `[RoomEventFilter](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3useruseridfilterfilterid_response-200_roomeventfilter)` | The state events to include for rooms. |
| `timeline` | `[RoomEventFilter](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3useruseridfilterfilterid_response-200_roomeventfilter)` | The message and state update events to include for rooms. |

#### RoomEventFilter

| Name | Type | Description |
| --- | --- | --- |
| `contains_url` | `boolean` | If `true`, includes only events with a `url` key in their content. If `false`, excludes those events. If omitted, `url` key is not considered for filtering. |
| `include_redundant_members` | `boolean` | If `true`, sends all membership events for all events, even if they have already been sent to the client. Does not apply unless `lazy_load_members` is `true`. See [Lazy-loading room members](https://spec.matrix.org/unstable/client-server-api/#lazy-loading-room-members) for more information. Defaults to `false`. |
| `lazy_load_members` | `boolean` | If `true`, enables lazy-loading of membership events. See [Lazy-loading room members](https://spec.matrix.org/unstable/client-server-api/#lazy-loading-room-members) for more information. Defaults to `false`. |
| `limit` | `integer` | The maximum number of events to return, must be an integer greater than 0.  Servers should apply a default value, and impose a maximum value to avoid resource exhaustion. |
| `not_rooms` | `[string]` | A list of room IDs to exclude. If this list is absent then no rooms are excluded. A matching room will be excluded even if it is listed in the `'rooms'` filter. |
| `not_senders` | `[string]` | A list of sender IDs to exclude. If this list is absent then no senders are excluded. A matching sender will be excluded even if it is listed in the `'senders'` filter. |
| `not_types` | `[string]` | A list of event types to exclude. If this list is absent then no event types are excluded. A matching type will be excluded even if it is listed in the `'types'` filter. A '\*' can be used as a wildcard to match any sequence of characters. |
| `rooms` | `[string]` | A list of room IDs to include. If this list is absent then all rooms are included. |
| `senders` | `[string]` | A list of senders IDs to include. If this list is absent then all senders are included. |
| `types` | `[string]` | A list of event types to include. If this list is absent then all event types are included. A `'*'` can be used as a wildcard to match any sequence of characters. |
| `unread_thread_notifications` | `boolean` | If `true`, enables per- [thread](https://spec.matrix.org/unstable/client-server-api/#threading) notification counts. Only applies to the `/sync` endpoint. Defaults to `false`.  **Added in `v1.4`** |

```json
{
  "event_fields": [
    "type",
    "content",
    "sender"
  ],
  "event_format": "client",
  "presence": {
    "not_senders": [
      "@alice:example.com"
    ],
    "types": [
      "m.presence"
    ]
  },
  "room": {
    "ephemeral": {
      "not_rooms": [
        "!726s6s6q:example.com"
      ],
      "not_senders": [
        "@spam:example.com"
      ],
      "types": [
        "m.receipt",
        "m.typing"
      ]
    },
    "state": {
      "not_rooms": [
        "!726s6s6q:example.com"
      ],
      "types": [
        "m.room.*"
      ]
    },
    "timeline": {
      "limit": 10,
      "not_rooms": [
        "!726s6s6q:example.com"
      ],
      "not_senders": [
        "@spam:example.com"
      ],
      "types": [
        "m.room.message"
      ]
    }
  }
}
```

## Events

The model of conversation history exposed by the client-server API can be considered as a list of events. The server 'linearises' the eventually-consistent event graph of events into an 'event stream' at any given point in time:

```
[E0]->[E1]->[E2]->[E3]->[E4]->[E5]
```

### Types of room events

Room events are split into two categories:

- **State events**: These are events which update the metadata state of the room (e.g. room topic, room membership etc). State is keyed by a tuple of event `type` and a `state_key`. State in the room with the same key-tuple will be overwritten.
- **Message events**: These are events which describe transient "once-off" activity in a room: typically communication such as sending an instant message or setting up a VoIP call.

This specification outlines several events, all with the event type prefix `m.`. (See [Room Events](https://spec.matrix.org/unstable/client-server-api/#room-events) for the m. event specification.) However, applications may wish to add their own type of event, and this can be achieved using the REST API detailed in the following sections. If new events are added, the event `type` key SHOULD follow the Java package naming convention, e.g.`com.example.myapp.event`. This ensures event types are suitably namespaced for each application and reduces the risk of clashes.

### Room event format

The "federation" format of a room event, which is used internally by homeservers and between homeservers via the Server-Server API, depends on the ["room version"](https://spec.matrix.org/unstable/rooms/) in use by the room. See, for example, the definitions in [room version 1](https://spec.matrix.org/unstable/rooms/v1/#event-format) and [room version 3](https://spec.matrix.org/unstable/rooms/v3/#event-format).

However, it is unusual that a Matrix client would encounter this event format. Instead, homeservers are responsible for converting events into the format shown below so that they can be easily parsed by clients.

## ClientEvent

---

The format used for events when they are returned from a homeserver to a client via the Client-Server API, or sent to an Application Service via the Application Services API.

| Name | Type | Description |
| --- | --- | --- |
| `content` | `object` | **Required:** The body of this event, as created by the client which sent it. |
| `event_id` | `string` | **Required:** The globally unique identifier for this event. |
| `origin_server_ts` | `integer` | **Required:** Timestamp (in milliseconds since the unix epoch) on originating homeserver when this event was sent. |
| `room_id` | `string` | **Required:** The ID of the room associated with this event. |
| `sender` | `string` | **Required:** Contains the fully-qualified ID of the user who sent this event. |
| `state_key` | `string` | Present if, and only if, this event is a *state* event. The key making this piece of state unique in the room. Note that it is often an empty string.  State keys starting with an `@` are reserved for referencing user IDs, such as room members. With the exception of a few events, state events set with a given user's ID as the state key MUST only be set by that user. |
| `type` | `string` | **Required:** The type of the event. |
| `unsigned` | `[UnsignedData](https://spec.matrix.org/unstable/client-server-api/#definition-clientevent_unsigneddata)` | Contains optional extra information about the event. |

#### UnsignedData

| Name | Type | Description |
| --- | --- | --- |
| `age` | `integer` | The time in milliseconds that has elapsed since the event was sent. This field is generated by the local homeserver, and may be incorrect if the local time on at least one of the two servers is out of sync, which can cause the age to either be negative or greater than it actually is. |
| `membership` | `string` | The room membership of the user making the request, at the time of the event.  This property is the value of the `membership` property of the requesting user's [`m.room.member`](https://spec.matrix.org/unstable/client-server-api/#mroommember) state at the point of the event, including any changes caused by the event. If the user had yet to join the room at the time of the event (i.e, they have no `m.room.member` state), this property is set to `leave`.  Homeservers SHOULD populate this property wherever practical, but they MAY omit it if necessary (for example, if calculating the value is expensive, servers might choose to only implement it in encrypted rooms). The property is *not* normally populated in events pushed to application services via the application service transaction API (where there is no clear definition of "requesting user").  **Added in `v1.11`** |
| `prev_content` | `EventContent` | The previous `content` for this event. This field is generated by the local homeserver, and is only returned if the event is a state event, and the client has permission to see the previous content.  **Changed in `v1.2`:** Previously, this field was specified at the top level of returned events rather than in `unsigned` (with the exception of the [`GET .../notifications`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3notifications) endpoint), though in practice no known server implementations honoured this. |
| `redacted_because` | `ClientEvent` | The event that redacted this event, if any. |
| `transaction_id` | `string` | The client-supplied [transaction ID](https://spec.matrix.org/unstable/client-server-api/#transaction-identifiers), for example, provided via `PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}`, if the client being given the event is the same one which sent it. |

## Examples

```json
{
  "content": {
    "membership": "join"
  },
  "event_id": "$26RqwJMLw-yds1GAH_QxjHRC1Da9oasK0e5VLnck_45",
  "origin_server_ts": 1632489532305,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@example:example.org",
  "state_key": "@user:example.org",
  "type": "m.room.member",
  "unsigned": {
    "age": 1567437,
    "membership": "join",
    "redacted_because": {
      "content": {
        "reason": "spam"
      },
      "event_id": "$Nhl3rsgHMjk-DjMJANawr9HHAhLg4GcoTYrSiYYGqEE",
      "origin_server_ts": 1632491098485,
      "redacts": "$26RqwJMLw-yds1GAH_QxjHRC1Da9oasK0e5VLnck_45",
      "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
      "sender": "@moderator:example.org",
      "type": "m.room.redaction",
      "unsigned": {
        "age": 1257,
        "membership": "leave"
      }
    }
  }
}
```

### Stripped state

Stripped state is a simplified view of the state of a room intended to help a potential joiner identify the room. It consists of a limited set of state events that are themselves simplified to reduce the amount of data required.

Stripped state events can only have the `sender`, `type`, `state_key` and `content` properties present.

Stripped state typically appears in invites, knocks, and in other places where a user *could* join the room under the conditions available (such as a [`restricted` room](https://spec.matrix.org/unstable/client-server-api/#restricted-rooms)).

Clients should only use stripped state events when they don't have access to the proper state of the room. Once the state of the room is available, all stripped state should be discarded. In cases where the client has an archived state of the room (such as after being kicked) and the client is receiving stripped state for the room, such as from an invite or knock, then the stripped state should take precedence until fresh state can be acquired from a join.

Stripped state should contain some or all of the following state events, which should be represented as stripped state events when possible:

- [`m.room.create`](https://spec.matrix.org/unstable/client-server-api/#mroomcreate)
- [`m.room.name`](https://spec.matrix.org/unstable/client-server-api/#mroomname)
- [`m.room.avatar`](https://spec.matrix.org/unstable/client-server-api/#mroomavatar)
- [`m.room.topic`](https://spec.matrix.org/unstable/client-server-api/#mroomtopic)
- [`m.room.join_rules`](https://spec.matrix.org/unstable/client-server-api/#mroomjoin_rules)
- [`m.room.canonical_alias`](https://spec.matrix.org/unstable/client-server-api/#mroomcanonical_alias)
- [`m.room.encryption`](https://spec.matrix.org/unstable/client-server-api/#mroomencryption)

## Stripped state event

---

A stripped down state event, with only the `type`, `state_key`,`sender`, and `content` keys.

| Name | Type | Description |
| --- | --- | --- |
| `content` | `EventContent` | **Required:** The `content` for the event. |
| `sender` | `string` | **Required:** The `sender` for the event. |
| `state_key` | `string` | **Required:** The `state_key` for the event. |
| `type` | `string` | **Required:** The `type` for the event. |

### Size limits

The complete event MUST NOT be larger than 65536 bytes, when formatted with the [federation event format](https://spec.matrix.org/unstable/client-server-api/#room-event-format), including any signatures, and encoded as [Canonical JSON](https://spec.matrix.org/unstable/appendices/#canonical-json).

There are additional restrictions on sizes per key:

- `sender` MUST NOT exceed the size limit for [user IDs](https://spec.matrix.org/unstable/appendices/#user-identifiers).
- `room_id` MUST NOT exceed the size limit for [room IDs](https://spec.matrix.org/unstable/appendices/#room-ids).
- `state_key` MUST NOT exceed 255 bytes.
- `type` MUST NOT exceed 255 bytes.
- `event_id` MUST NOT exceed the size limit for [event IDs](https://spec.matrix.org/unstable/appendices/#event-ids).

Some event types have additional size restrictions which are specified in the description of the event. Additional keys have no limit other than that implied by the total 64 KiB limit on events.

### Room Events

This specification outlines several standard event types, all of which are prefixed with `m.`

## m.room.canonical_alias

---

This event is used to inform the room about which alias should be considered the canonical one, and which other aliases point to the room. This could be for display purposes or as suggestion to users which alias to use to advertise and access the room.

| Event type: | State event |
| --- | --- |
| State key | A zero-length string. |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `alias` | `string` | The canonical alias for the room. If not present, null, or empty the room should be considered to have no canonical alias. |
| `alt_aliases` | `[string]` | Alternative aliases the room advertises. This list can have aliases despite the `alias` field being null, empty, or otherwise not present. |

### Examples

```json
{
  "content": {
    "alias": "#somewhere:localhost",
    "alt_aliases": [
      "#somewhere:example.org",
      "#myroom:example.com"
    ]
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@example:example.org",
  "state_key": "",
  "type": "m.room.canonical_alias",
  "unsigned": {
    "age": 1234,
    "membership": "join"
  }
}
```

## m.room.create

---

This is the first event in a room and cannot be changed. It acts as the root of all other events.

| Event type: | State event |
| --- | --- |
| State key | A zero-length string. |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `additional_creators` | `[string]` | Starting with room version 12, the other user IDs to consider as creators for the room in addition to the `sender` of this event. Each string MUST be a valid [user ID](https://spec.matrix.org/unstable/appendices/#user-identifiers) for the room version.  When not present or empty, the `sender` of the event is the only creator.  In room versions 1 through 11, this field serves no purpose and is not validated. Clients SHOULD NOT attempt to parse or understand this field in these room versions.  **Note**: Because `creator` was removed in room version 11, the field is not used to determine which user(s) are room creators in room version 12 and beyond either.  **Added in `v1.16`** |
| `creator` | `string` | The `user_id` of the room creator. **Required** for, and only present in, room versions 1 - 10. Starting with room version 11 the event `sender` should be used instead. |
| `m.federate` | `boolean` | Whether users on other servers can join this room. Defaults to `true` if key does not exist. |
| `predecessor` | `[Previous Room](https://spec.matrix.org/unstable/client-server-api/#mroomcreate_previous-room)` | A reference to the room this room replaces, if the previous room was upgraded. |
| `room_version` | `string` | The version of the room. Defaults to `"1"` if the key does not exist. |
| `type` | `string` | Optional [room type](https://spec.matrix.org/unstable/client-server-api/#types) to denote a room's intended function outside of traditional conversation.  Unspecified room types are possible using [Namespaced Identifiers](https://spec.matrix.org/unstable/appendices/#common-namespaced-identifier-grammar). |

#### Previous Room

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | The event ID of the last known event in the old room, if known.  If not set, clients SHOULD search for the `m.room.tombstone` state event to navigate to when directing the user to the old room (potentially after joining the room, if requested by the user).  **Changed in `v1.16`:** This field became deprecated and may not be present in all cases. It SHOULD still be populated where possible/practical. Previously, it was required. |
| `room_id` | `string` | **Required:** The ID of the old room. |

### Examples

```json
{
  "content": {
    "m.federate": true,
    "predecessor": {
      "event_id": "$something:example.org",
      "room_id": "!oldroom:example.org"
    },
    "room_version": "11"
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@example:example.org",
  "state_key": "",
  "type": "m.room.create",
  "unsigned": {
    "age": 1234,
    "membership": "join"
  }
}
```

## m.room.join_rules

---

A room may have one of the following designations:

- `public` - anyone can join the room without any prior action.
- `invite` - a user must first receive an invite from someone already in the room in order to join.
- `knock` - a user can request an invite to the room. They can be allowed (invited) or denied (kicked/banned) access. Otherwise, users need to be invited in. Only available in rooms [which support knocking](https://spec.matrix.org/unstable/rooms/#feature-matrix).
- `restricted` - anyone able to satisfy at least one of the allow conditions is able to join the room without prior action. Otherwise, an invite is required. Only available in rooms [which support the join rule](https://spec.matrix.org/unstable/rooms/#feature-matrix).
- `knock_restricted` - a user can request an invite using the same functions offered by the `knock` join rule, or can attempt to join having satisfied an allow condition per the `restricted` join rule. Only available in rooms [which support the join rule](https://spec.matrix.org/unstable/rooms/#feature-matrix).
- `private` - reserved without implementation. No significant meaning.
| Event type: | State event |
| --- | --- |
| State key | A zero-length string. |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `allow` | `[[AllowCondition](https://spec.matrix.org/unstable/client-server-api/#mroomjoin_rules_allowcondition)]` | For `restricted` rooms, the conditions the user will be tested against. The user needs only to satisfy one of the conditions to join the `restricted` room. If the user fails to meet any condition, or the condition is unable to be confirmed as satisfied, then the user requires an invite to join the room. Improper or no `allow` conditions on a `restricted` join rule imply the room is effectively invite-only (no conditions can be satisfied).  **Added in `v1.2`** |
| `join_rule` | `string` | **Required:** The type of rules used for users wishing to join this room.  One of: `[public, knock, invite, private, restricted, knock_restricted]`. |

#### AllowCondition

| Name | Type | Description |
| --- | --- | --- |
| `room_id` | `string` | Required if `type` is `m.room_membership`. The room ID to check the user's membership against. If the user is joined to this room, they satisfy the condition and thus are permitted to join the `restricted` room. |
| `type` | `string` | **Required:** The type of condition:  - `m.room_membership` - the user satisfies the condition if they are joined to the referenced room.  One of: `[m.room_membership]`. |

### Examples

```json
{
  "content": {
    "join_rule": "public"
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@example:example.org",
  "state_key": "",
  "type": "m.room.join_rules",
  "unsigned": {
    "age": 1234,
    "membership": "join"
  }
}
```

```json
{
  "content": {
    "allow": [
      {
        "room_id": "!other:example.org",
        "type": "m.room_membership"
      },
      {
        "room_id": "!elsewhere:example.org",
        "type": "m.room_membership"
      }
    ],
    "join_rule": "restricted"
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@example:example.org",
  "state_key": "",
  "type": "m.room.join_rules",
  "unsigned": {
    "age": 1234,
    "membership": "join"
  }
}
```

## m.room.member

---

Adjusts the membership state for a user in a room. It is preferable to use the membership APIs (`/rooms/<room id>/invite` etc) when performing membership actions rather than adjusting the state directly as there are a restricted set of valid transformations. For example, user A cannot force user B to join a room, and trying to force this state change directly will fail.

The following membership states are specified:

- `invite` - The user has been invited to join a room, but has not yet joined it. They may not participate in the room until they join.
- `join` - The user has joined the room (possibly after accepting an invite), and may participate in it.
- `leave` - The user was once joined to the room, but has since left (possibly by choice, or possibly by being kicked).
- `ban` - The user has been banned from the room, and is no longer allowed to join it until they are un-banned from the room (by having their membership state set to a value other than `ban`).
- `knock` - The user has knocked on the room, requesting permission to participate. They may not participate in the room until they join.

The `third_party_invite` property will be set if this invite is an `invite` event and is the successor of an [`m.room.third_party_invite`](https://spec.matrix.org/unstable/client-server-api/#mroomthird_party_invite) event, and absent otherwise.

This event may also include an `invite_room_state` key inside the event's `unsigned` data. If present, this contains an array of [stripped state events](https://spec.matrix.org/unstable/client-server-api/#stripped-state) to assist the receiver in identifying the room.

The user for which a membership applies is represented by the `state_key`. Under some conditions, the `sender` and `state_key` may not match - this may be interpreted as the `sender` affecting the membership state of the `state_key` user.

The `membership` for a given user can change over time. The table below represents the various changes over time and how clients and servers must interpret those changes. Previous membership can be retrieved from the `prev_content` object on an event. If not present, the user's previous membership must be assumed as `leave`.

|  | to `invite` | to `join` | to `leave` | to `ban` | to `knock` |
| --- | --- | --- | --- | --- | --- |
| **from `invite`** | No change. | User joined the room. | If the `state_key` is the same as the `sender`, the user rejected the invite. Otherwise, the `state_key` user had their invite revoked. | User was banned. | User is re-knocking. |
| **from `join`** | Must never happen. | `displayname` or `avatar_url` changed. | If the `state_key` is the same as the `sender`, the user left. Otherwise, the `state_key` user was kicked. | User was kicked and banned. | Must never happen. |
| **from `leave`** | New invitation sent. | User joined. | No change. | User was banned. | User is knocking. |
| **from `ban`** | Must never happen. | Must never happen. | User was unbanned. | No change. | Must never happen. |
| **from `knock`** | Knock accepted. | Must never happen. | If the `state_key` is the same as the `sender`, the user retracted the knock. Otherwise, the `state_key` user had their knock denied. | User was banned. | No change. |

| Event type: | State event |
| --- | --- |
| State key | The `user_id` this membership event relates to. In all cases except for when `membership` is `join`, the user ID sending the event does not need to match the user ID in the `state_key`, unlike other events. Regular authorisation rules still apply. |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `avatar_url` | `[URI](https://datatracker.ietf.org/doc/html/rfc3986)` | The avatar URL for this user, if any. |
| `displayname` | `string\|null` | The display name for this user, if any. |
| `is_direct` | `boolean` | Flag indicating if the room containing this event was created with the intention of being a direct chat. See [Direct Messaging](https://spec.matrix.org/unstable/client-server-api/#direct-messaging). |
| `join_authorised_via_users_server` | `string` | Usually found on `join` events, this field is used to denote which homeserver (through representation of a user with sufficient power level) authorised the user's join. More information about this field can be found in the [Restricted Rooms Specification](https://spec.matrix.org/unstable/client-server-api/#restricted-rooms).  Client and server implementations should be aware of the of including this field in further events: in particular, the event must be signed by the server which owns the user ID in the field. When copying the membership event's `content` (for profile updates and similar) it is therefore encouraged to exclude this field in the copy, as otherwise the event might fail event authorization.  **Added in `v1.2`** |
| `membership` | `string` | **Required:** The membership state of the user.  One of: `[invite, join, knock, leave, ban]`. |
| `reason` | `string` | Optional user-supplied text for why their membership has changed. For kicks and bans, this is typically the reason for the kick or ban. For other membership changes, this is a way for the user to communicate their intent without having to send a message to the room, such as in a case where Bob rejects an invite from Alice about an upcoming concert, but can't make it that day.  Clients are not recommended to show this reason to users when receiving an invite due to the potential for spam and abuse. Hiding the reason behind a button or other component is recommended.  **Added in `v1.1`** |
| `third_party_invite` | `[ThirdPartyInvite](https://spec.matrix.org/unstable/client-server-api/#mroommember_thirdpartyinvite)` | A third-party invite, if this `m.room.member` is the successor to an [`m.room.third_party_invite`](https://spec.matrix.org/unstable/client-server-api/#mroomthird_party_invite) event. |

#### ThirdPartyInvite

| Name | Type | Description |
| --- | --- | --- |
| `display_name` | `string` | **Required:** A name which can be displayed to represent the user instead of their third-party identifier |
| `signed` | `[SignedThirdPartyInvite](https://spec.matrix.org/unstable/client-server-api/#mroommember_signedthirdpartyinvite)` | **Required:** A block of content which has been signed by the identity server, which homeservers can use to verify the event. Clients should ignore this. |

#### SignedThirdPartyInvite

| Name | Type | Description |
| --- | --- | --- |
| `mxid` | `[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers)` | **Required:** The user ID that has been bound to the third-party identifier. |
| `signatures` | `{string: {string: string}}` | **Required:** The identity server signatures for this block. This is a map of identity server name to signing key identifier to base64-encoded signature.  The signatures are calculated using the process described at [Signing JSON](https://spec.matrix.org/unstable/appendices/#signing-json). |
| `token` | `string` | **Required:** The token generated by the identity server at the [`/store_invite`](https://spec.matrix.org/unstable/identity-service-api/#post_matrixidentityv2store-invite) endpoint.  It matches the `state_key` of the corresponding [`m.room.third_party_invite`](https://spec.matrix.org/unstable/client-server-api/#mroomthird_party_invite) event. |

### Examples

```json
{
  "content": {
    "avatar_url": "mxc://example.org/SEsfnsuifSDFSSEF",
    "displayname": "Alice Margatroid",
    "membership": "join",
    "reason": "Looking for support"
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@alice:example.org",
  "state_key": "@alice:example.org",
  "type": "m.room.member",
  "unsigned": {
    "age": 1234,
    "membership": "join"
  }
}
```

```json
{
  "content": {
    "avatar_url": "mxc://example.org/SEsfnsuifSDFSSEF",
    "displayname": "Alice Margatroid",
    "membership": "invite",
    "reason": "Looking for support"
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@alice:example.org",
  "state_key": "@alice:example.org",
  "type": "m.room.member",
  "unsigned": {
    "age": 1234,
    "invite_room_state": [
      {
        "content": {
          "name": "Example Room"
        },
        "sender": "@bob:example.org",
        "state_key": "",
        "type": "m.room.name"
      },
      {
        "content": {
          "join_rule": "invite"
        },
        "sender": "@bob:example.org",
        "state_key": "",
        "type": "m.room.join_rules"
      }
    ]
  }
}
```

```json
{
  "content": {
    "avatar_url": "mxc://example.org/SEsfnsuifSDFSSEF",
    "displayname": "Alice Margatroid",
    "join_authorised_via_users_server": "@bob:other.example.org",
    "membership": "join"
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@alice:example.org",
  "state_key": "@alice:example.org",
  "type": "m.room.member",
  "unsigned": {
    "age": 1234
  }
}
```

```json
{
  "content": {
    "avatar_url": "mxc://example.org/SEsfnsuifSDFSSEF",
    "displayname": "Alice Margatroid",
    "membership": "knock",
    "reason": "Looking for support"
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@alice:example.org",
  "state_key": "@alice:example.org",
  "type": "m.room.member",
  "unsigned": {
    "age": 1234,
    "knock_room_state": [
      {
        "content": {
          "name": "Example Room"
        },
        "sender": "@bob:example.org",
        "state_key": "",
        "type": "m.room.name"
      },
      {
        "content": {
          "join_rule": "knock"
        },
        "sender": "@bob:example.org",
        "state_key": "",
        "type": "m.room.join_rules"
      }
    ]
  }
}
```

```json
{
  "content": {
    "avatar_url": "mxc://example.org/SEsfnsuifSDFSSEF",
    "displayname": "Alice Margatroid",
    "membership": "invite",
    "third_party_invite": {
      "display_name": "alice",
      "signed": {
        "mxid": "@alice:example.org",
        "signatures": {
          "magic.forest": {
            "ed25519:3": "fQpGIW1Snz+pwLZu6sTy2aHy/DYWWTspTJRPyNp0PKkymfIsNffysMl6ObMMFdIJhk6g6pwlIqZ54rxo8SLmAg"
          }
        },
        "token": "abc123"
      }
    }
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@alice:example.org",
  "state_key": "@alice:example.org",
  "type": "m.room.member",
  "unsigned": {
    "age": 1234,
    "membership": "join"
  }
}
```

## m.room.power_levels

---

This event specifies the minimum level a user must have in order to perform a certain action. It also specifies the levels of each user in the room.

If a `user_id` is in the `users` list, then that `user_id` has the associated power level. Otherwise they have the default level `users_default`. If `users_default` is not supplied, it is assumed to be 0. If the room contains no `m.room.power_levels` event, the room's creator has a power level of 100, and all other users have a power level of 0.

The level required to send a certain event is governed by `events`,`state_default` and `events_default`. If an event type is specified in `events`, then the user must have at least the level specified in order to send that event. If the event type is not supplied, it defaults to `events_default` for Message Events and `state_default` for State Events.

If there is no `state_default` in the `m.room.power_levels` event, or there is no `m.room.power_levels` event, the `state_default` is 50. If there is no `events_default` in the `m.room.power_levels` event, or there is no `m.room.power_levels` event, the `events_default` is 0.

The power level required to invite a user to the room, kick a user from the room, ban a user from the room, or redact an event sent by another user, is defined by `invite`, `kick`, `ban`, and `redact`, respectively. The levels for `kick`, `ban` and `redact` default to 50 if they are not specified in the `m.room.power_levels` event, or if the room contains no `m.room.power_levels` event. `invite` defaults to 0 in either case.

**Note:**

The allowed range for power level values is `[-(2**53)+1, (2**53)-1]`, as required by the [Canonical JSON specification](https://spec.matrix.org/unstable/appendices/#canonical-json).

| Event type: | State event |
| --- | --- |
| State key | A zero-length string. |

### Content

| Name | Type | Description |
| --- | --- | --- |
| `ban` | `integer` | The level required to ban a user. Defaults to 50 if unspecified. |
| `events` | `{string: integer}` | The level required to send specific event types. This is a mapping from event type to power level required.  Though not a default, when the server sends the initial power levels event during [room creation](https://spec.matrix.org/unstable/client-server-api/#creation) in [room versions](https://spec.matrix.org/unstable/rooms/) 12 and higher, the `m.room.tombstone` event MUST be explicitly defined and given a power level higher than `state_default`. For example, power level 150. Clients may override this using the described `power_level_content_override` field.  **Changed in `v1.16`:** Described `m.room.tombstone` defaults during creation of a room version 12 or higher room. |
| `events_default` | `integer` | The default level required to send message events. Can be overridden by the `events` key. Defaults to 0 if unspecified. |
| `invite` | `integer` | The level required to invite a user. Defaults to 0 if unspecified. |
| `kick` | `integer` | The level required to kick a user. Defaults to 50 if unspecified. |
| `notifications` | `[Notifications](https://spec.matrix.org/unstable/client-server-api/#mroompower_levels_notifications)` | The power level requirements for specific notification types. This is a mapping from `key` to power level for that notifications key. |
| `redact` | `integer` | The level required to redact an event sent by another user. Defaults to 50 if unspecified. |
| `state_default` | `integer` | The default level required to send state events. Can be overridden by the `events` key. Defaults to 50 if unspecified. |
| `users` | `{[User ID](https://spec.matrix.org/unstable/appendices#user-identifiers): integer}` | The power levels for specific users. This is a mapping from `user_id` to power level for that user.  **Note**: In [room versions](https://spec.matrix.org/unstable/rooms/) 12 and higher it is not permitted to specify the room creators here.  **Changed in `v1.16`:** Added a note that room creators cannot be specified here in room versions 12 and higher. |
| `users_default` | `integer` | The power level for users in the room whose `user_id` is not mentioned in the `users` key. Defaults to 0 if unspecified.  **Note**: In [room versions](https://spec.matrix.org/unstable/rooms/) 1 through 11, when there is no `m.room.power_levels` event in the room, the room creator has a power level of 100, and all other users have a power level of 0.  **Note**: In room versions 12 and higher, room creators have infinite power level regardless of the existence of `m.room.power_levels` in the room. When `m.room.power_levels` is not in the room however, all other users have a power level of 0.  **Changed in `v1.16`:** The room creator power level now changes depending on room version. |

#### Notifications

| Name | Type | Description |
| --- | --- | --- |
| `room` | `integer` | The level required to trigger an `@room` notification. Defaults to 50 if unspecified. |
| <Other properties> | `integer` |  |

### Examples

```json
{
  "content": {
    "ban": 50,
    "events": {
      "m.room.name": 100,
      "m.room.power_levels": 100
    },
    "events_default": 0,
    "invite": 50,
    "kick": 50,    "notifications": {
      "room": 20
    },
    "redact": 50,
    "state_default": 50,
    "users": {
      "@example:localhost": 100
    },
    "users_default": 0
  },
  "event_id": "$143273582443PhrSn:example.org",
  "origin_server_ts": 1432735824653,
  "room_id": "!jEsUZKDJdhlrceRyVU:example.org",
  "sender": "@example:example.org",
  "state_key": "",
  "type": "m.room.power_levels",
  "unsigned": {
    "age": 1234,
    "membership": "join"
  }
}
```

#### Historical events

Some events within the `m.` namespace might appear in rooms, however they serve no significant meaning in this version of the specification. They are:

- `m.room.aliases`

Previous versions of the specification have more information on these events.

### Syncing

To read events, the intended flow of operation is for clients to first call the [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync) API without a `since` parameter. This returns the most recent message events for each room, as well as the state of the room at the start of the returned timeline. The response also includes a `next_batch` field, which should be used as the value of the `since` parameter in the next call to `/sync`. Finally, the response includes, for each room, a `prev_batch` field, which can be passed as a `from` / `to` parameter to the [`/rooms/<room_id>/messages`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3roomsroomidmessages) API to retrieve earlier messages.

For example, a `/sync` request might return a range of four events `E2`, `E3`, `E4` and `E5` within a given room, omitting two prior events `E0` and `E1`. This can be visualised as follows:

```
[E0]->[E1]->[E2]->[E3]->[E4]->[E5]
               ^                      ^
               |                      |
         prev_batch: '1-2-3'        next_batch: 'a-b-c'
```

Clients then receive new events by "long-polling" the homeserver via the `/sync` API, passing the value of the `next_batch` field from the response to the previous call as the `since` parameter. The client should also pass a `timeout` parameter. The server will then hold open the HTTP connection for a short period of time waiting for new events, returning early if an event occurs. Only the `/sync` API (and the deprecated `/events` API) support long-polling in this way.

Continuing the example above, an incremental sync might report a single new event `E6`. The response can be visualised as:

```
[E0]->[E1]->[E2]->[E3]->[E4]->[E5]->[E6]
                                      ^     ^
                                      |     |
                                      |  next_batch: 'x-y-z'
                                    prev_batch: 'a-b-c'
```

Normally, all new events which are visible to the client will appear in the response to the `/sync` API. However, if a large number of events arrive between calls to `/sync`, a "limited" timeline is returned, containing only the most recent message events. A state "delta" is also returned, summarising any state changes in the omitted part of the timeline. The client may therefore end up with "gaps" in its knowledge of the message timeline. The client can fill these gaps using the [`/rooms/<room_id>/messages`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3roomsroomidmessages) API.

Continuing our example, suppose we make a third `/sync` request asking for events since the last sync, by passing the `next_batch` token `x-y-z` as the `since` parameter. The server knows about four new events, `E7`, `E8`,`E9` and `E10`, but decides this is too many to report at once. Instead, the server sends a `limited` response containing `E8`, `E9` and `E10` but omitting `E7`. This forms a gap, which we can see in the visualisation:

```
| gap |
                                            | <-> |
    [E0]->[E1]->[E2]->[E3]->[E4]->[E5]->[E6]->[E7]->[E8]->[E9]->[E10]
                                            ^     ^                  ^
                                            |     |                  |
                                 since: 'x-y-z'   |                  |
                                       prev_batch: 'd-e-f'       next_batch: 'u-v-w'
```

The limited response includes a state delta which describes how the state of the room changes over the gap. This delta explains how to build the state prior to returned timeline (i.e. at `E7`) from the state the client knows (i.e. at `E6`). To close the gap, the client should make a request to [`/rooms/<room_id>/messages`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3roomsroomidmessages) with the query parameters `from=x-y-z` and `to=d-e-f`.

## GET /_matrix/client/v3/sync

---

Synchronise the client's state with the latest state on the server. Clients use this API when they first log in to get an initial snapshot of the state on the server, and then continue to call this API to get incremental deltas to the state, and to receive new messages.

*Note*: This endpoint supports lazy-loading. See [Filtering](https://spec.matrix.org/unstable/client-server-api/#filtering) for more information. Lazy-loading members is only supported on the `state` part of a [`RoomFilter`](https://spec.matrix.org/unstable/client-server-api/#post_matrixclientv3useruseridfilter_request_roomfilter) for this endpoint. When lazy-loading is enabled, servers MUST include the syncing user's own membership event when they join a room, or when the full state of rooms is requested, to aid discovering the user's avatar & displayname.

Further, like other members, the user's own membership event is eligible for being considered redundant by the server. When a sync is `limited`, the server MUST return membership events for events in the gap (between `since` and the start of the returned timeline), regardless as to whether or not they are redundant. This ensures that joins/leaves and profile changes which occur during the gap are not lost.

Note that the default behaviour of `state` is to include all membership events, alongside other state, when lazy-loading is not enabled.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

### Request

#### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `filter` | `string` | The ID of a filter created using the filter API or a filter JSON object encoded as a string. The server will detect whether it is an ID or a JSON object by whether the first character is a `"{"` open brace. Passing the JSON inline is best suited to one off requests. Creating a filter using the filter API is recommended for clients that reuse the same filter multiple times, for example in long poll requests.  See [Filtering](https://spec.matrix.org/unstable/client-server-api/#filtering) for more information. |
| `full_state` | `boolean` | Controls whether to include the full state for all rooms the user is a member of.  If this is set to `true`, then all state events will be returned, even if `since` is non-empty. The timeline will still be limited by the `since` parameter. In this case, the `timeout` parameter will be ignored and the query will return immediately, possibly with an empty timeline.  If `false`, and `since` is non-empty, only state which has changed since the point indicated by `since` will be returned.  By default, this is `false`. |
| `set_presence` | `string` | Controls whether the client is automatically marked as online by polling this API. If this parameter is omitted then the client is automatically marked as online when it uses this API. Otherwise if the parameter is set to "offline" then the client is not marked as being online when it uses this API. When set to "unavailable", the client is marked as being idle.  One of: `[offline, online, unavailable]`. |
| `since` | `string` | A point in time to continue a sync from. This should be the `next_batch` token returned by an earlier call to this endpoint. |
| `timeout` | `integer` | The maximum time to wait, in milliseconds, before returning this request. If no events (or other data) become available before this time elapses, the server will return a response with empty fields.  By default, this is `0`, so the server will return immediately even if the response is empty. |
| `use_state_after` | `boolean` | Controls whether to receive state changes between the previous sync and the **start** of the timeline, or between the previous sync and the **end** of the timeline.  If this is set to `true`, servers MUST respond with the state between the previous sync and the **end** of the timeline in `state_after` and MUST omit `state`.  If `false`, servers MUST respond with the state between the previous sync and the **start** of the timeline in `state` and MUST omit `state_after`.  Even if this is set to `true`, clients MUST update their local state with events in `state` and `timeline` if `state_after` is missing in the response, for compatibility with servers that don't support this parameter.  By default, this is `false`.  **Added in `v1.16`** |

### Responses

| Status | Description |
| --- | --- |
| `200` | The initial snapshot or delta for the client to use to update their state. |

#### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `account_data` | `[Account Data](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync_response-200_account-data)` | The global private data created by this user. |
| `device_lists` | `DeviceLists` | Information on end-to-end device updates, as specified in [End-to-end encryption](https://spec.matrix.org/unstable/client-server-api/#e2e-extensions-to-sync). |
| `device_one_time_keys_count` | `{string: integer}` | Information on end-to-end encryption keys, as specified in [End-to-end encryption](https://spec.matrix.org/unstable/client-server-api/#e2e-extensions-to-sync). |
| `next_batch` | `string` | **Required:** The batch token to supply in the `since` param of the next `/sync` request. |
| `presence` | `[Presence](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync_response-200_presence)` | The updates to the presence status of other users. |
| `rooms` | `[Rooms](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync_response-200_rooms)` | Updates to rooms. |
| `to_device` | `ToDevice` | Information on the send-to-device messages for the client device, as defined in [Send-to-Device messaging](https://spec.matrix.org/unstable/client-server-api/#extensions-to-sync). |