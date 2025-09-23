# Matrix Server-Server API - Public Rooms and Space Hierarchy

This specification covers the published room directory federation endpoints and space hierarchy functionality, allowing homeservers to query published rooms and space information from other servers.

## Published Room Directory

To complement the [room directory in the Client-Server API](https://spec.matrix.org/unstable/client-server-api/#published-room-directory), homeservers need a way to query the published rooms of another server. This can be done by making a request to the `/publicRooms` endpoint for the server the room directory should be retrieved for.

