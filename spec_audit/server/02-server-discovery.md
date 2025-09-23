# Server Discovery

## Resolving Server Names

Each Matrix homeserver is identified by a server name consisting of a hostname and an optional port, as described by the grammar. Where applicable, a delegated server name uses the same grammar.

Server names are resolved to an IP address and port to connect to, and have various conditions affecting which certificates and `Host` headers to send. The process overall is as follows:

### Resolution Process

1. **IP Literal with Port**: If the hostname is an IP literal, then that IP address should be used, together with the given port number, or 8448 if no port is given. The target server must present a valid certificate for the IP address. The `Host` header in the request should be set to the server name, including the port if the server name included one.

2. **Hostname with Explicit Port**: If the hostname is not an IP literal, and the server name includes an explicit port, resolve the hostname to an IP address using CNAME, AAAA or A records. Requests are made to the resolved IP address and given port with a `Host` header of the original server name (with port). The target server must present a valid certificate for the hostname.

3. **Well-known Discovery**: If the hostname is not an IP literal, a regular HTTPS request is made to `https://<hostname>/.well-known/matrix/server`, expecting the schema defined below. 30x redirects should be followed, however redirection loops should be avoided.

   **Caching**: Responses (successful or otherwise) to the `/.well-known` endpoint should be cached by the requesting server. Servers should respect the cache control headers present on the response, or use a sensible default when headers are not present. The recommended sensible default is 24 hours. Servers should additionally impose a maximum cache time for responses: 48 hours is recommended. Errors are recommended to be cached for up to an hour, and servers are encouraged to exponentially back off for repeated failures.

   If the response is invalid (bad JSON, missing properties, non-200 response, etc), skip to step 4. If the response is valid, the `m.server` property is parsed as `<delegated_hostname>[:<delegated_port>]` and processed as follows:

   - **Delegated IP Literal**: If `<delegated_hostname>` is an IP literal, then that IP address should be used together with the `<delegated_port>` or 8448 if no port is provided. The target server must present a valid TLS certificate for the IP address. Requests must be made with a `Host` header containing the IP address, including the port if one was provided.

   - **Delegated Hostname with Port**: If `<delegated_hostname>` is not an IP literal, and `<delegated_port>` is present, an IP address is discovered by looking up CNAME, AAAA or A records for `<delegated_hostname>`. The resulting IP address is used, alongside the `<delegated_port>`. Requests must be made with a `Host` header of `<delegated_hostname>:<delegated_port>`. The target server must present a valid certificate for `<delegated_hostname>`.

   - **[Added in v1.8] Delegated SRV Lookup**: If `<delegated_hostname>` is not an IP literal and no `<delegated_port>` is present, an SRV record is looked up for `_matrix-fed._tcp.<delegated_hostname>`. This may result in another hostname (to be resolved using AAAA or A records) and port. Requests should be made to the resolved IP address and port with a `Host` header containing the `<delegated_hostname>`. The target server must present a valid certificate for `<delegated_hostname>`.

   - **[Deprecated] Legacy SRV Lookup**: If `<delegated_hostname>` is not an IP literal, no `<delegated_port>` is present, and a `_matrix-fed._tcp.<delegated_hostname>` SRV record was not found, an SRV record is looked up for `_matrix._tcp.<delegated_hostname>`. This may result in another hostname (to be resolved using AAAA or A records) and port. Requests should be made to the resolved IP address and port with a `Host` header containing the `<delegated_hostname>`. The target server must present a valid certificate for `<delegated_hostname>`.

   - **Direct Resolution**: If no SRV record is found, an IP address is resolved using CNAME, AAAA or A records. Requests are then made to the resolved IP address and a port of 8448, using a `Host` header of `<delegated_hostname>`. The target server must present a valid certificate for `<delegated_hostname>`.

4. **[Added in v1.8] Fallback SRV Lookup**: If the `/.well-known` request resulted in an error response, a server is found by resolving an SRV record for `_matrix-fed._tcp.<hostname>`. This may result in a hostname (to be resolved using AAAA or A records) and port. Requests are made to the resolved IP address and port, with a `Host` header of `<hostname>`. The target server must present a valid certificate for `<hostname>`.

5. **[Deprecated] Legacy Fallback SRV**: If the `/.well-known` request resulted in an error response, and a `_matrix-fed._tcp.<hostname>` SRV record was not found, a server is found by resolving an SRV record for `_matrix._tcp.<hostname>`. This may result in a hostname (to be resolved using AAAA or A records) and port. Requests are made to the resolved IP address and port, with a `Host` header of `<hostname>`. The target server must present a valid certificate for `<hostname>`.

6. **Final Fallback**: If the `/.well-known` request returned an error response, and no SRV records were found, an IP address is resolved using CNAME, AAAA and A records. Requests are made to the resolved IP address using port 8448 and a `Host` header containing the `<hostname>`. The target server must present a valid certificate for `<hostname>`.

## Well-known Server Information

### GET /.well-known/matrix/server

Gets information about the delegated server for server-server communication between Matrix homeservers. Servers should follow 30x redirects, carefully avoiding redirect loops, and use normal X.509 certificate validation.

| Rate-limited: | No |
| Requires authentication: | No |

#### Request
No request parameters or request body.

#### Response (200)
The delegated server information. The `Content-Type` for this response SHOULD be `application/json`, however servers parsing the response should assume that the body is JSON regardless of type. Failures parsing the JSON or invalid data provided in the resulting parsed JSON should not result in discovery failure - consult the server discovery process for information on how to continue.

**Response Format:**
```json
{
  "m.server": "delegated.example.com:1234"
}
```

**Fields:**
- `m.server` (string, required): The server name to delegate server-server communications to, with optional port. The delegated server name uses the same grammar as server names in the appendices.