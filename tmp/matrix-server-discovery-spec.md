# Matrix Server Discovery Specification

## Overview
Matrix server discovery is the process by which homeservers determine the actual network location of other homeservers for federation requests. This is critical for proper Matrix federation functionality.

## Discovery Process (Priority Order)

### 1. Well-Known Discovery (Primary Method)
**Endpoint**: `GET https://<hostname>/.well-known/matrix/server`

**Request**:
- Method: GET
- URL: `https://<hostname>/.well-known/matrix/server`
- Host Header: `<hostname>`
- User-Agent: Should identify the requesting server

**Response Format**:
```json
{
  "m.server": {
    "base_url": "https://<delegated_hostname>:<port>"
  }
}
```

**Response Handling**:
- **200 OK**: Use the `base_url` for federation requests
- **404 Not Found**: Proceed to SRV record lookup
- **Other Status Codes**: Proceed to SRV record lookup
- **Invalid JSON**: Proceed to SRV record lookup
- **Missing m.server field**: Proceed to SRV record lookup

**TLS Requirements**:
- Federation requests made to `<delegated_hostname>:<port>`
- Host header must be `<hostname>` (original server name)
- Target server must present valid certificate for `<hostname>`

### 2. SRV Record Discovery (Fallback)
**Primary SRV**: `_matrix-fed._tcp.<hostname>`
**Deprecated SRV**: `_matrix._tcp.<hostname>` (may be removed)

**Process**:
1. DNS SRV lookup for `_matrix-fed._tcp.<hostname>`
2. If not found, try deprecated `_matrix._tcp.<hostname>`
3. SRV record resolves to `<delegated_hostname>:<port>`
4. Make federation requests to resolved IP and port
5. Use Host header `<hostname>`
6. Target server must present valid certificate for `<hostname>`

### 3. Direct IP Resolution (Final Fallback)
**Process**:
1. DNS CNAME, AAAA, A record lookup for `<hostname>`
2. Resolve to IP address
3. Make federation requests to `<ip_address>:8448`
4. Use Host header `<hostname>`
5. Target server must present valid certificate for `<hostname>`

## Caching Requirements

### HTTP Caching Standards
Well-known responses should be cached according to HTTP caching standards:

**Cache-Control Header**:
- Respect `max-age` directive
- Default TTL: 24 hours if no cache headers
- Maximum TTL: 48 hours (hard limit)

**Expires Header**:
- Use if Cache-Control not present
- Parse using HTTP date format (RFC 7231)

**Error Caching**:
- Cache failed requests for up to 1 hour
- Implement exponential backoff for repeated failures

### Redirect Handling
- Follow 30x redirects (301, 302, 307, 308)
- Maximum 10 redirects to prevent loops
- Update URL for subsequent requests in redirect chain

## Security Considerations

### Certificate Validation
- Always validate TLS certificates
- Certificate must match original `<hostname>`, not delegated hostname
- Prevent man-in-the-middle attacks through proper certificate validation

### Request Validation
- Validate JSON response structure
- Handle malformed responses gracefully
- Implement proper timeout handling

## Implementation Notes

### Performance
- Cache successful and failed lookups appropriately
- Use connection pooling for HTTP requests
- Implement circuit breaker patterns for failing servers

### Error Handling
- Graceful degradation through fallback methods
- Proper logging for debugging federation issues
- Clear error messages for troubleshooting

## References
- Matrix Server-Server API Specification
- RFC 8615 (Well-Known URIs)
- RFC 7231 (HTTP/1.1 Semantics and Content)
- RFC 2782 (DNS SRV Records)