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

