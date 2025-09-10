# Transactions

The transfer of EDUs and PDUs between homeservers is performed by an exchange of Transaction messages, which are encoded as JSON objects, passed over an HTTP PUT request. A Transaction is meaningful only to the pair of homeservers that exchanged it; they are not globally-meaningful.

Transactions are limited in size; they can have at most 50 PDUs and 100 EDUs.

## PUT /\_matrix/federation/v1/send/{txnId}

Push messages representing live activity to another server. The destination name will be set to that of the receiving server itself. Each embedded PDU in the transaction body will be processed.

The sending server must wait and retry for a 200 OK response before sending a transaction with a different `txnId` to the receiving server.

Note that events have a different format depending on the room version - check the room version specification for precise event formats.

| Rate-limited: | No |
| Requires authentication: | Yes |

### Request Parameters
- `txnId` (string, required): The transaction ID.

### Request Body
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

**Fields:**
- `origin` (string, required): The `server_name` of the homeserver sending this transaction
- `origin_server_ts` (integer, required): POSIX timestamp in milliseconds on originating homeserver when this transaction started
- `pdus` (array, required): List of persistent updates to rooms. Must not include more than 50 PDUs. Note that events have a different format depending on the room version
- `edus` (array): List of ephemeral messages. May be omitted if there are no ephemeral messages to be sent. Must not include more than 100 EDUs

**EDU Format:**
- `edu_type` (string, required): The type of ephemeral message
- `content` (object, required): The content of the ephemeral message

### Response (200)
The result of processing the transaction. The server is to use this response even in the event of one or more PDUs failing to be processed.

```json
{
  "pdus": {
    "$failed_event:example.org": {
      "error": "You are not allowed to send a message to this room."
    },
    "$successful_event:example.org": {}
  }
}
```

**Fields:**
- `pdus` (object, required): The PDUs from the original transaction. The string key represents the ID of the PDU (event) that was processed.
- `pdus[event_id].error` (string): A human readable description about what went wrong in processing this PDU. If no error is present, the PDU can be considered successfully handled.