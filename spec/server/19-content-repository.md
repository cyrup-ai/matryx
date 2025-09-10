# Matrix Server-Server API: Content Repository

*Federation protocol specification for media content distribution in the Matrix ecosystem.*

---

## Overview

The Content Repository enables servers to federate media content (images, files, etc.) across the Matrix network. This specification defines the endpoints and protocols for downloading media from remote servers.

---

## Content Repository

Attachments to events (images, files, etc) are uploaded to a homeserver via the Content Repository described in the [Client-Server API](https://spec.matrix.org/unstable/client-server-api/#content-repository). When a server wishes to serve content originating from a remote server, it needs to ask the remote server for the media.

Servers MUST use the server described in the [Matrix Content URI](https://spec.matrix.org/unstable/client-server-api/#matrix-content-mxc-uris). Formatted as `mxc://{ServerName}/{MediaID}`, servers MUST download the media from `ServerName` using the below endpoints.

**\[Changed in `v1.11`\]** Servers were previously advised to use the `/_matrix/media/*` endpoints described by the [Content Repository module in the Client-Server API](https://spec.matrix.org/unstable/client-server-api/#content-repository), however, those endpoints have been deprecated. New endpoints are introduced which require authentication. Naturally, as a server is not a user, they cannot provide the required access token to those endpoints. Instead, servers MUST try the endpoints described below before falling back to the deprecated `/_matrix/media/*` endpoints when they receive a `404 M_UNRECOGNIZED` error. When falling back, servers MUST be sure to set `allow_remote` to `false`.

## GET /\_matrix/federation/v1/media/download/{mediaId}

---

**Added in `v1.11`**

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

---

## Request### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `mediaId` | `string` | **Required:** The media ID from the `mxc://` URI (the path component). |

| Name | Type | Description |
| --- | --- | --- |
| `timeout_ms` | `integer` | The maximum number of milliseconds that the client is willing to wait to start receiving data, in the case that the content has not yet been uploaded. The default value is 20000 (20 seconds). The content repository SHOULD impose a maximum value for this parameter. The content repository MAY respond before the timeout.  **Added in `v1.7`** |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The content that was previously uploaded. |
| `429` | This request was rate-limited. |
| `502` | The content is too large for the server to serve. |
| `504` | The content is not yet available. A [standard error response](https://spec.matrix.org/unstable/client-server-api/#standard-error-response) will be returned with the `errcode` `M_NOT_YET_UPLOADED`. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `Content-Type` | `string` | Must be `multipart/mixed`. |

| Content-Type | Description |
| --- | --- |
| `multipart/mixed` | **Required.** MUST contain a `boundary` (per [RFC 2046](https://datatracker.ietf.org/doc/html/rfc2046#section-5.1)) delineating exactly two parts:  The first part has a `Content-Type` header of `application/json` and describes the media's metadata, if any. Currently, this will always be an empty object.  The second part is either:  1. the bytes of the media itself, using `Content-Type` and `Content-Disposition` headers as appropriate; 2. or a `Location` header to redirect the caller to where the media can be retrieved. The URL at `Location` SHOULD have appropriate `Content-Type` and `Content-Disposition` headers which describe the media. 	When `Location` is present, servers SHOULD NOT cache the URL. The remote server may have applied time limits on its validity. If the caller requires an up-to-date URL, it SHOULD re-request the media download. |

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
```### 502 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "errcode": "M_TOO_LARGE",

  "error": "Content is too large to serve"

}
```

### 504 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "errcode": "M_NOT_YET_UPLOADED",

  "error": "Content has not yet been uploaded"

}
```

## GET /\_matrix/federation/v1/media/thumbnail/{mediaId}

---

**Added in `v1.11`**

Download a thumbnail of content from the content repository. See the [Client-Server API Thumbnails](https://spec.matrix.org/unstable/client-server-api/#thumbnails) section for more information.

| Rate-limited: | Yes |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `mediaId` | `string` | **Required:** The media ID from the `mxc://` URI (the path component). |

| Name | Type | Description |
| --- | --- | --- || `animated` | `boolean` | Indicates preference for an animated thumbnail from the server, if possible. Animated thumbnails typically use the content types `image/gif`, `image/png` (with APNG format),`image/apng`, and `image/webp` instead of the common static `image/png` or `image/jpeg` content types.  When `true`, the server SHOULD return an animated thumbnail if possible and supported. When `false`, the server MUST NOT return an animated thumbnail. For example, returning a static `image/png` or `image/jpeg` thumbnail. When not provided, the server SHOULD NOT return an animated thumbnail.  Servers SHOULD prefer to return `image/webp` thumbnails when supporting animation.  When `true` and the media cannot be animated, such as in the case of a JPEG or PDF, the server SHOULD behave as though `animated` is `false`.  **Added in `v1.11`** |
| `height` | `integer` | **Required:** The *desired* height of the thumbnail. The actual thumbnail may be larger than the size specified. |
| `method` | `string` | The desired resizing method. See the [Client-Server API Thumbnails](https://spec.matrix.org/unstable/client-server-api/#thumbnails) section for more information.  One of: `[crop, scale]`. |
| `timeout_ms` | `integer` | The maximum number of milliseconds that the client is willing to wait to start receiving data, in the case that the content has not yet been uploaded. The default value is 20000 (20 seconds). The content repository SHOULD impose a maximum value for this parameter. The content repository MAY respond before the timeout.  **Added in `v1.7`** |
| `width` | `integer` | **Required:** The *desired* width of the thumbnail. The actual thumbnail may be larger than the size specified. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | A thumbnail of the requested content. |
| `400` | The request does not make sense to the server, or the server cannot thumbnail the content. For example, the caller requested non-integer dimensions or asked for negatively-sized images. |
| `413` | The local content is too large for the server to thumbnail. |
| `429` | This request was rate-limited. |
| `502` | The remote content is too large for the server to thumbnail. |
| `504` | The content is not yet available. A [standard error response](https://spec.matrix.org/unstable/client-server-api/#standard-error-response) will be returned with the `errcode` `M_NOT_YET_UPLOADED`. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `Content-Type` | `string` | Must be `multipart/mixed`. |

| Content-Type | Description |
| --- | --- |
| `multipart/mixed` | **Required.** MUST contain a `boundary` (per [RFC 2046](https://datatracker.ietf.org/doc/html/rfc2046#section-5.1)) delineating exactly two parts:  The first part has a `Content-Type` header of `application/json` and describes the media's metadata, if any. Currently, this will always be an empty object.  The second part is either:  1. the bytes of the media itself, using `Content-Type` and `Content-Disposition` headers as appropriate; 2. or a `Location` header to redirect the caller to where the media can be retrieved. The URL at `Location` SHOULD have appropriate `Content-Type` and `Content-Disposition` headers which describe the media. 	When `Location` is present, servers SHOULD NOT cache the URL. The remote server may have applied time limits on its validity. If the caller requires an up-to-date URL, it SHOULD re-request the media download. |

### 400 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "errcode": "M_UNKNOWN",

  "error": "Cannot generate thumbnails for the requested content"

}
```### 413 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "errcode": "M_TOO_LARGE",

  "error": "Content is too large to thumbnail"

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

### 502 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "errcode": "M_TOO_LARGE",

  "error": "Content is too large to thumbnail"

}
```### 504 response

| Name | Type | Description |
| --- | --- | --- |
| `errcode` | `string` | **Required:** An error code. |
| `error` | `string` | A human-readable error message. |

```json
{

  "errcode": "M_NOT_YET_UPLOADED",

  "error": "Content has not yet been uploaded"

}
```