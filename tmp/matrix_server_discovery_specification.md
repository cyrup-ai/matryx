# Matrix Server-Server API DNS Resolution Specification

## Overview

Matrix homeservers must implement proper DNS resolution for federation according to the Matrix Server-Server API specification. This document outlines the complete server discovery process.

## Server Discovery Process

Each Matrix homeserver is identified by a server name consisting of a hostname and an optional port. The resolution process follows these steps in order:

### 1. IP Literal Handling

If the hostname is an IP literal:
- Use that IP address with the given port (or 8448 if no port)
- Target server must present valid certificate for the IP address
- Host header should be set to the server name (including port if present)

### 2. Explicit Port Handling

If hostname is not IP literal and server name includes explicit port:
- Resolve hostname using CNAME, AAAA, or A records
- Connect to resolved IP with given port
- Host header should be original server name (with port)
- Target server must present valid certificate for hostname

### 3. Well-Known Delegation

If no explicit port, make HTTPS request to:
```
https://<hostname>/.well-known/matrix/server
```

Expected response format:
```json
{
  "m.server": "<delegated_hostname>[:<delegated_port>]"
}
```

Caching requirements:
- Follow 30x redirects (avoid loops)
- Cache responses respecting Cache-Control headers
- Default cache time: 24 hours
- Maximum cache time: 48 hours (recommended)
- Cache errors for up to 1 hour
- Use exponential backoff for repeated failures

If well-known succeeds, process delegated server name recursively.

### 4. SRV Record Lookup (Matrix v1.8+)

If well-known fails or returns error, lookup SRV record:
```
_matrix-fed._tcp.<hostname>
```

SRV record format:
```
_matrix-fed._tcp.example.com. 300 IN SRV 10 5 8448 matrix.example.com.
                                      ^  ^ ^    ^
                                      |  | |    target hostname
                                      |  | port
                                      |  weight (higher = preferred)
                                      priority (lower = higher priority)
```

### 5. Legacy SRV Record Lookup (Deprecated)

If `_matrix-fed._tcp` SRV record not found, try legacy:
```
_matrix._tcp.<hostname>
```

### 6. Fallback to Port 8448

If no SRV records found:
- Resolve hostname using CNAME, AAAA, A records
- Connect to resolved IP on port 8448
- Host header should be hostname
- Target server must present valid certificate for hostname

## SRV Record Priority and Weight Handling

When multiple SRV records exist:
1. Sort by priority (lower number = higher priority)
2. Within same priority, use weighted random selection
3. Try records in order until successful connection

## DNS Record Types

### A Records (IPv4)
```
example.com. 300 IN A 192.168.1.100
```

### AAAA Records (IPv6)
```
example.com. 300 IN AAAA 2001:db8::1
```

### CNAME Records (Canonical Name)
```
matrix.example.com. 300 IN CNAME server.example.com.
```

### SRV Records (Service Records)
```
_matrix-fed._tcp.example.com. 300 IN SRV 10 5 8448 matrix.example.com.
```

## Implementation Requirements

### DNS Resolver Features
- Support for A, AAAA, CNAME, and SRV record lookups
- Proper priority/weight handling for SRV records
- Timeout handling and error recovery
- Caching with appropriate TTL respect

### Well-Known Client Features
- HTTPS request handling with redirect support
- Response caching with Cache-Control header respect
- Error caching and exponential backoff
- JSON parsing and validation

### Integration Requirements
- Replace direct hostname usage in federation URLs
- Use resolved IP addresses for connections
- Set proper Host headers for HTTP requests
- Validate TLS certificates against correct hostnames

## Security Considerations

### Certificate Validation
- IP literals: validate against IP address
- Hostname resolution: validate against original hostname
- Well-known delegation: validate against delegated hostname

### Host Header Security
- Always set Host header to expected value
- Prevent Host header injection attacks
- Use original server name for Host header (not resolved IP)

### DNS Security
- Implement DNS timeout handling
- Use secure DNS resolvers when possible
- Handle DNS poisoning gracefully
- Implement proper error handling and fallbacks

## Error Handling

### DNS Resolution Errors
- Network timeouts
- NXDOMAIN responses
- No records found
- Invalid record formats

### Well-Known Errors
- HTTP errors (4xx, 5xx)
- Invalid JSON responses
- Missing required fields
- Redirect loops

### Fallback Strategy
Always implement complete fallback chain:
1. Well-known delegation
2. _matrix-fed._tcp SRV records
3. _matrix._tcp SRV records (legacy)
4. Direct hostname:8448 connection

## Testing Recommendations

### DNS Record Setup
```bash
# Modern SRV record (Matrix v1.8+)
_matrix-fed._tcp.example.com. 300 IN SRV 10 5 8448 matrix.example.com.

# Legacy SRV record (deprecated but still supported)
_matrix._tcp.example.com. 300 IN SRV 10 5 8448 matrix.example.com.

# Target hostname resolution
matrix.example.com. 300 IN A 192.168.1.100
```

### Well-Known Setup
```bash
# Serve at https://example.com/.well-known/matrix/server
{
  "m.server": "matrix.example.com:8448"
}
```

### Test Cases
- IP literal server names
- Explicit port server names  
- Well-known delegation
- SRV record resolution
- Fallback scenarios
- Error conditions

## References

- [Matrix Server-Server API Specification](https://spec.matrix.org/v1.11/server-server-api/)
- [RFC 2782: DNS SRV Records](https://tools.ietf.org/html/rfc2782)
- [RFC 1035: Domain Names](https://tools.ietf.org/html/rfc1035)
- [Matrix Well-Known Specification](https://spec.matrix.org/v1.11/server-server-api/#resolving-server-names)