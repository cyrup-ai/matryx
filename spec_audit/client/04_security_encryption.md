# Matrix Client-Server API: Security and Encryption

This document contains the security and encryption features from the Matrix Client-Server specification, including send-to-device messaging, device management, and end-to-end encryption.

## Send-to-Device messaging

This module provides a means by which clients can exchange signalling messages without them being stored permanently as part of a shared communication history. A message is delivered exactly once to each client device.

The primary motivation for this API is exchanging data that is meaningless or undesirable to persist in the room DAG - for example, one-time authentication tokens or key data. It is not intended for conversational data, which should be sent using the normal [`/rooms/<room_id>/send`](https://spec.matrix.org/unstable/client-server-api/#put_matrixclientv3roomsroomidsendeventtypetxnid) API for consistency throughout Matrix.

### Client behaviour

To send a message to other devices, a client should call [`/sendToDevice`](https://spec.matrix.org/unstable/client-server-api/#put_matrixclientv3sendtodeviceeventtypetxnid). Only one message can be sent to each device per transaction, and they must all have the same event type. The device ID in the request body can be set to `*` to request that the message be sent to all known devices.

If there are send-to-device messages waiting for a client, they will be returned by [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync), as detailed in [Extensions to /sync](https://spec.matrix.org/unstable/client-server-api/#extensions-to-sync). Clients should inspect the `type` of each returned event, and ignore any they do not understand.

### Server behaviour

Servers should store pending messages for local users until they are successfully delivered to the destination device. When a client calls [`/sync`](https://spec.matrix.org/unstable/client-server-api/#get_matrixclientv3sync) with an access token which corresponds to a device with pending messages, the server should list the pending messages, in order of arrival, in the response body.

When the client calls `/sync` again with the `next_batch` token from the first response, the server should infer that any send-to-device messages in that response have been delivered successfully, and delete them from the store.

If there is a large queue of send-to-device messages, the server should limit the number sent in each `/sync` response. 100 messages is recommended as a reasonable limit.

If the client sends messages to users on remote domains, those messages should be sent on to the remote servers via [federation](https://spec.matrix.org/unstable/server-server-api/#send-to-device-messaging).

### Protocol definitions

