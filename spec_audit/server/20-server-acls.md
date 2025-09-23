# Matrix Server-Server API: Server Access Control Lists (ACLs)

*Federation protocol specification for server access control in Matrix rooms.*

---

## Overview

Server ACLs enable room administrators to control which homeservers can participate in federation for specific rooms. This specification defines how servers must validate and enforce ACL restrictions.

---

## Server Access Control Lists (ACLs)

Server ACLs and their purpose are described in the [Server ACLs](https://spec.matrix.org/unstable/client-server-api/#server-access-control-lists-acls-for-rooms) section of the Client-Server API.

When a remote server makes a request, it MUST be verified to be allowed by the server ACLs. If the server is denied access to a room, the receiving server MUST reply with a 403 HTTP status code and an `errcode` of `M_FORBIDDEN`.

The following endpoint prefixes MUST be protected:

- `/_matrix/federation/v1/make_join`
- `/_matrix/federation/v1/make_leave`
- `/_matrix/federation/v1/send_join`
- `/_matrix/federation/v2/send_join`
- `/_matrix/federation/v1/send_leave`
- `/_matrix/federation/v2/send_leave`
- `/_matrix/federation/v1/invite`
- `/_matrix/federation/v2/invite`
- `/_matrix/federation/v1/make_knock`
- `/_matrix/federation/v1/send_knock`
- `/_matrix/federation/v1/state`
- `/_matrix/federation/v1/state_ids`
- `/_matrix/federation/v1/backfill`
- `/_matrix/federation/v1/event_auth`
- `/_matrix/federation/v1/get_missing_events`

Additionally the [`/_matrix/federation/v1/send/{txnId}`](https://spec.matrix.org/unstable/server-server-api/#put_matrixfederationv1sendtxnid) endpoint MUST be protected as follows:

- ACLs MUST be applied to all PDUs on a per-PDU basis. If the sending server is denied access to the room identified by `room_id`, the PDU MUST be ignored with an appropriate error included in the response for the respective event ID.
- ACLs MUST be applied to all EDUs that are local to a specific room:
	- For [typing notifications (`m.typing`)](https://spec.matrix.org/unstable/server-server-api/#typing-notifications), if the sending server is denied access to the room identified by `room_id`, the EDU MUST be ignored.
	- For [receipts (`m.receipt`)](https://spec.matrix.org/unstable/server-server-api/#receipts), all receipts for a particular room ID MUST be ignored if the sending server is denied access to the room identified by that ID.