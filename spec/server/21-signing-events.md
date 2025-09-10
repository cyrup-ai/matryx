# Matrix Server-Server API: Event Signing and Validation

*Federation protocol specification for cryptographic event signatures and hash validation in Matrix.*

---

## Overview

Event signing ensures the integrity and authenticity of Matrix events during federation. This specification defines the algorithms for creating, validating, and processing cryptographic signatures and hashes.

---

## Signing Events

Signing events is complicated by the fact that servers can choose to redact non-essential parts of an event.

### Adding hashes and signatures to outgoing events

Before signing the event, the *content hash* of the event is calculated as described below. The hash is encoded using [Unpadded Base64](https://spec.matrix.org/unstable/appendices/#unpadded-base64) and stored in the event object, in a `hashes` object, under a `sha256` key.

The event object is then *redacted*, following the [redaction algorithm](https://spec.matrix.org/unstable/client-server-api/#redactions). Finally it is signed as described in [Signing JSON](https://spec.matrix.org/unstable/appendices/#signing-json), using the server's signing key (see also [Retrieving server keys](https://spec.matrix.org/unstable/server-server-api/#retrieving-server-keys)).

The signature is then copied back to the original event object.

For an example of a signed event, see the [room version specification](https://spec.matrix.org/unstable/rooms/).

### Validating hashes and signatures on received events

When a server receives an event over federation from another server, the receiving server should check the hashes and signatures on that event.

First the signature is checked. The event is redacted following the [redaction algorithm](https://spec.matrix.org/unstable/client-server-api/#redactions), and the resultant object is checked for a signature from the originating server, following the algorithm described in [Checking for a signature](https://spec.matrix.org/unstable/appendices/#checking-for-a-signature). Note that this step should succeed whether we have been sent the full event or a redacted copy.

The signatures expected on an event are:

- The `sender` 's server, unless the invite was created as a result of 3rd party invite. The sender must already match the 3rd party invite, and the server which actually sends the event may be a different server.
- For room versions 1 and 2, the server which created the `event_id`. Other room versions do not track the `event_id` over federation and therefore do not need a signature from those servers.

If the signature is found to be valid, the expected content hash is calculated as described below. The content hash in the `hashes` property of the received event is base64-decoded, and the two are compared for equality.

If the hash check fails, then it is assumed that this is because we have only been given a redacted version of the event. To enforce this, the receiving server should use the redacted copy it calculated rather than the full copy it received.### Calculating the reference hash for an event

The *reference hash* of an event covers the essential fields of an event, including content hashes. It is used for event identifiers in some room versions. See the [room version specification](https://spec.matrix.org/unstable/rooms/) for more information. It is calculated as follows.

1. The event is put through the redaction algorithm.
2. The `signatures` and `unsigned` properties are removed from the event, if present.
3. The event is converted into [Canonical JSON](https://spec.matrix.org/unstable/appendices/#canonical-json).
4. A sha256 hash is calculated on the resulting JSON object.

### Calculating the content hash for an event

The *content hash* of an event covers the complete event including the *unredacted* contents. It is calculated as follows.

First, any existing `unsigned`, `signatures`, and `hashes` properties are removed. The resulting object is then encoded as [Canonical JSON](https://spec.matrix.org/unstable/appendices/#canonical-json), and the JSON is hashed using SHA-256.

### Example code

```py
def hash_and_sign_event(event_object, signing_key, signing_name):

    # First we need to hash the event object.

    content_hash = compute_content_hash(event_object)

    event_object["hashes"] = {"sha256": encode_unpadded_base64(content_hash)}

    # Strip all the keys that would be removed if the event was redacted.

    # The hashes are not stripped and cover all the keys in the event.

    # This means that we can tell if any of the non-essential keys are

    # modified or removed.

    stripped_object = strip_non_essential_keys(event_object)    # Sign the stripped JSON object. The signature only covers the

    # essential keys and the hashes. This means that we can check the

    # signature even if the event is redacted.

    signed_object = sign_json(stripped_object, signing_key, signing_name)

    # Copy the signatures from the stripped event to the original event.

    event_object["signatures"] = signed_object["signatures"]

def compute_content_hash(event_object):

    # take a copy of the event before we remove any keys.

    event_object = dict(event_object)

    # Keys under "unsigned" can be modified by other servers.

    # They are useful for conveying information like the age of an

    # event that will change in transit.

    # Since they can be modified we need to exclude them from the hash.

    event_object.pop("unsigned", None)

    # Signatures will depend on the current value of the "hashes" key.

    # We cannot add new hashes without invalidating existing signatures.

    event_object.pop("signatures", None)

    # The "hashes" key might contain multiple algorithms if we decide to

    # migrate away from SHA-2. We don't want to include an existing hash

    # output in our hash so we exclude the "hashes" dict from the hash.

    event_object.pop("hashes", None)

    # Encode the JSON using a canonical encoding so that we get the same

    # bytes on every server for the same JSON object.

    event_json_bytes = encode_canonical_json(event_object)

    return hashlib.sha256(event_json_bytes)
```

## Security considerations

When a domain's ownership changes, the new controller of the domain can masquerade as the previous owner, receiving messages (similarly to email) and request past messages from other servers. In the future, proposals like [MSC1228](https://github.com/matrix-org/matrix-spec-proposals/issues/1228) will address this issue.