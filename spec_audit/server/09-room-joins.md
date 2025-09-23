# Room Joining

## Overview

Room joining in Matrix federation involves a multi-step handshake process between the joining server and a resident server in the target room. This process ensures proper authorization and provides the joining server with the necessary room state information.

## Room Joining Flow

The room joining process follows this sequence:

```
+---------+          +---------------+            +-----------------+ +-----------------+
| Client  |          | Joining       |            | Directory       | | Resident        |
|         |          | Server        |            | Server          | | Server          |
+---------+          +---------------+            +-----------------+ +-----------------+
     |                       |                             |                   |
     | join request          |                             |                   |
     |---------------------->|                             |                   |
     |                       |                             |                   |
     |                       | directory request           |                   |
     |                       |---------------------------->|                   |
     |                       |                             |                   |
     |                       |          directory response |                   |
     |                       |<----------------------------|                   |
     |                       |                             |                   |
     |                       | make_join request           |                   |
     |                       |------------------------------------------------>|
     |                       |                             |                   |
     |                       |                             |make_join response |
     |                       |<------------------------------------------------|
     |                       |                             |                   |
     |                       | send_join request           |                   |
     |                       |------------------------------------------------>|
     |                       |                             |                   |
     |                       |                             |send_join response |
     |                       |<------------------------------------------------|
     |                       |                             |                   |
     |         join response |                             |                   |
     |<----------------------|                             |                   |
     |                       |                             |                   |
```

## Joining Process Steps

### 1. Directory Resolution

The first part of the handshake usually involves using the directory server to request the room ID and join candidates through the [`/query/directory`](https://spec.matrix.org/unstable/server-server-api/#get_matrixfederationv1querydirectory) API endpoint. In the case of a new user joining a room as a result of a received invite, the joining user's homeserver could optimise this step away by picking the origin server of that invite message as the join candidate. However, the joining server should be aware that the origin server of the invite might since have left the room, so should be prepared to fall back on the regular join flow if this optimisation fails.

### 2. Make Join Request

Once the joining server has the room ID and the join candidates, it then needs to obtain enough information about the room to fill in the required fields of the `m.room.member` event. It obtains this by selecting a resident from the candidate list, and using the `GET /make_join` endpoint. The resident server will then reply with enough information for the joining server to fill in the event.

### 3. Event Preparation

The joining server is expected to add or replace the `origin`, `origin_server_ts`, and `event_id` on the templated event received by the resident server. This event is then signed by the joining server.

### 4. Send Join Request

To complete the join handshake, the joining server submits this new event to the resident server it used for `GET /make_join`, using the `PUT /send_join` endpoint.

### 5. Event Acceptance

The resident homeserver then adds its signature to this event and accepts it into the room's event graph. The joining server receives the full set of state for the newly-joined room as well as the freshly signed membership event. The resident server must also send the event to other servers participating in the room.

