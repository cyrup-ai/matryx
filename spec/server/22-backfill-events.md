# Matrix Server-Server API: Backfilling and Retrieving Missing Events

*Federation protocol specification for historical event retrieval in Matrix rooms.*

---

## Overview

Backfilling enables homeservers to retrieve historical events from other servers when users request room history that predates the server's membership. This specification defines the APIs for fetching missing events and room history.

---

## Backfilling and retrieving missing events

Once a homeserver has joined a room, it receives all the events emitted by other homeservers in that room, and is thus aware of the entire history of the room from that moment onwards. Since users in that room are able to request the history by the `/messages` client API endpoint, it's possible that they might step backwards far enough into history before the homeserver itself was a member of that room.

To cover this case, the federation API provides a server-to-server analog of the `/messages` client API, allowing one homeserver to fetch history from another. This is the `/backfill` API.

To request more history, the requesting homeserver picks another homeserver that it thinks may have more (most likely this should be a homeserver for some of the existing users in the room at the earliest point in history it has currently), and makes a `/backfill` request.

Similar to backfilling a room's history, a server may not have all the events in the graph. That server may use the `/get_missing_events` API to acquire the events it is missing.## GET /\_matrix/federation/v1/backfill/{roomId}

---

Retrieves a sliding-window history of previous PDUs that occurred in the given room. Starting from the PDU ID(s) given in the `v` argument, the PDUs given in `v` and the PDUs that preceded them are retrieved, up to the total number given by the `limit`.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room ID to backfill. |

| Name | Type | Description |
| --- | --- | --- |
| `limit` | `integer` | **Required:** The maximum number of PDUs to retrieve, including the given events. |
| `v` | `[string]` | **Required:** The event IDs to backfill from. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | A transaction containing the PDUs that preceded the given event(s), including the given event(s), up to the given limit.  **Note:**Though the PDU definitions require that `prev_events` and `auth_events` be limited in number, the response of backfill MUST NOT be validated on these specific restrictions.  Due to historical reasons, it is possible that events which were previously accepted would now be rejected by these limitations. The events should be rejected per usual by the `/send`, `/get_missing_events`, and remaining endpoints. |### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `origin` | `string` | **Required:** The `server_name` of the homeserver sending this transaction. |
| `origin_server_ts` | `integer` | **Required:** POSIX timestamp in milliseconds on originating homeserver when this transaction started. |
| `pdus` | `[PDU]` | **Required:** List of persistent updates to rooms. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. |

```json
{

  "origin": "matrix.org",

  "origin_server_ts": 1234567890,

  "pdus": [

    {

      "content": {

        "see_room_version_spec": "The event format changes depending on the room version."

      },

      "room_id": "!somewhere:example.org",

      "type": "m.room.minimal_pdu"

    }

  ]

}
```

## POST /\_matrix/federation/v1/get\_missing\_events/{roomId}

---

Retrieves previous events that the sender is missing. This is done by doing a breadth-first walk of the `prev_events` for the `latest_events`, ignoring any events in `earliest_events` and stopping at the `limit`.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room ID to search in. |

### Request body

| Name | Type | Description |
| --- | --- | --- |
| `earliest_events` | `[string]` | **Required:** The latest event IDs that the sender already has. These are skipped when retrieving the previous events of `latest_events`. |
| `latest_events` | `[string]` | **Required:** The event IDs to retrieve the previous events for. |
| `limit` | `integer` | The maximum number of events to retrieve. Defaults to 10. |
| `min_depth` | `integer` | The minimum depth of events to retrieve. Defaults to 0. |

### Request body example

```json
{

  "earliest_events": [

    "$missing_event:example.org"

  ],

  "latest_events": [

    "$event_that_has_the_missing_event_as_a_previous_event:example.org"

  ],

  "limit": 10

}
```

---## Responses

| Status | Description |
| --- | --- |
| `200` | The previous events for `latest_events`, excluding any `earliest_events`, up to the provided `limit`. |

### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `events` | `[PDU]` | **Required:** The missing events. The event format varies depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. |

```json
{

  "events": [

    {

      "content": {

        "see_room_version_spec": "The event format changes depending on the room version."

      },

      "room_id": "!somewhere:example.org",

      "type": "m.room.minimal_pdu"

    }

  ]

}
```