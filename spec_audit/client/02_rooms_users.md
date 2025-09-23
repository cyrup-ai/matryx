# Matrix Client-Server API - Rooms and Users Management

This section covers the Matrix Client-Server API endpoints and concepts related to room creation, membership management, room aliases, and user permissions.

## Room Creation

- `m.room.power_levels`: Sets the power levels of users and required power levels for various actions in the room. This is a state event.
- `m.room.join_rules`: Whether the room is "invite-only" or not.

See [Room Events](https://spec.matrix.org/unstable/client-server-api/#room-events) for more information on these events. To create a room, a client has to use the following API.

