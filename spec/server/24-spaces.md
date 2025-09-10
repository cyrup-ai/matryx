# Matrix Server-Server API: Spaces

*Federation protocol specification for Matrix Spaces hierarchy queries.*

---

## Overview

Spaces enable hierarchical organization of Matrix rooms. This specification defines the federation endpoints for querying space hierarchies from remote servers.

---

## Spaces

To complement the [Client-Server API's Spaces module](https://spec.matrix.org/unstable/client-server-api/#spaces), homeservers need a way to query information about spaces from other servers.

## GET /\_matrix/federation/v1/hierarchy/{roomId}

---

**Added in `v1.2`**

Federation version of the Client-Server [`GET /hierarchy`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv1roomsroomidhierarchy) endpoint. Unlike the Client-Server API version, this endpoint does not paginate. Instead, all the space-room's children the requesting server could feasibly peek/join are returned. The requesting server is responsible for filtering the results further down for the user's request.

Only [`m.space.child`](https://spec.matrix.org/unstable/client-server-api/#mspacechild) state events of the room are considered. Invalid child rooms and parent events are not covered by this endpoint.

Responses to this endpoint should be cached for a period of time.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---