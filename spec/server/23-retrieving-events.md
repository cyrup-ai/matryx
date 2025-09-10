# Matrix Server-Server API: Retrieving Events

*Federation protocol specification for single event and room state retrieval in Matrix.*

---

## Overview

Event retrieval APIs enable homeservers to fetch specific events and room state snapshots from other servers when backfilling is insufficient. This specification defines the endpoints for retrieving individual events and room state.

---

## Retrieving events

In some circumstances, a homeserver may be missing a particular event or information about the room which cannot be easily determined from backfilling. These APIs provide homeservers with the option of getting events and the state of the room at a given point in the timeline.

## GET /\_matrix/federation/v1/event/{eventId}

---

Retrieves a single event.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `eventId` | `string` | **Required:** The event ID to get. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | A transaction containing a single PDU which is the event requested. |### 200 response

| Name | Type | Description |
| --- | --- | --- |
| `origin` | `string` | **Required:** The `server_name` of the homeserver sending this transaction. |
| `origin_server_ts` | `integer` | **Required:** POSIX timestamp in milliseconds on originating homeserver when this transaction started. |
| `pdus` | `[PDU]` | **Required:** A single PDU. Note that events have a different format depending on the room version - check the [room version specification](https://spec.matrix.org/unstable/rooms/) for precise event formats. |

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

## GET /\_matrix/federation/v1/state/{roomId}

---

Retrieves a snapshot of a room's state at a given event.

| Rate-limited: | No |
| --- | --- |
| Requires authentication: | Yes |

---

## Request

### Request parameters

| Name | Type | Description |
| --- | --- | --- |
| `roomId` | `string` | **Required:** The room ID to get state for. |

| Name | Type | Description |
| --- | --- | --- |
| `event_id` | `string` | **Required:** An event ID in the room to retrieve the state at. |

---

## Responses

| Status | Description |
| --- | --- |
| `200` | The fully resolved state for the room, prior to considering any state changes induced by the requested event. Includes the authorization chain for the events. |