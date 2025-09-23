# Matrix Server-Server API: End-to-End Encryption

**Section 27 of the Matrix Server-Server API specification**

This section covers end-to-end encryption federation capabilities in the Matrix Server-Server API, including key claiming, key querying, and cross-signing key updates.

---

## End-to-End Encryption

This section complements the [End-to-End Encryption module](https://spec.matrix.org/unstable/client-server-api/#end-to-end-encryption) of the Client-Server API. For detailed information about end-to-end encryption, please see that module.

The APIs defined here are designed to be able to proxy much of the client's request through to federation, and have the response also be proxied through to the client.

