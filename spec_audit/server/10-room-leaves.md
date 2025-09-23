# Room Leaving Federation

This document covers the federation aspects of leaving Matrix rooms, including rejecting invites and the associated server-to-server API endpoints.

## Overview

Normally homeservers can send appropriate `m.room.member` events to have users leave the room, to reject local invites, or to retract a knock. Remote invites/knocks from other homeservers do not involve the server in the graph and therefore need another approach to reject the invite. Joining the room and promptly leaving is not recommended as clients and servers will interpret that as accepting the invite, then leaving the room rather than rejecting the invite.

Similar to the [Joining Rooms](https://spec.matrix.org/unstable/server-server-api/#joining-rooms) handshake, the server which wishes to leave the room starts with sending a `/make_leave` request to a resident server. In the case of rejecting invites, the resident server may be the server which sent the invite. After receiving a template event from `/make_leave`, the leaving server signs the event and replaces the `event_id` with its own. This is then sent to the resident server via `/send_leave`. The resident server will then send the event to other servers in the room.

