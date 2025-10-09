# Matrix v1.3 Destination Parameter Specification

## X-Matrix Authorization Header Format

```
Authorization: X-Matrix origin="origin.hs.example.com",destination="destination.hs.example.com",key="ed25519:key1",sig="ABCDEF..."
```

## Requirements (Matrix v1.3+)

1. **Destination Parameter**: Added in Matrix v1.3
   - MUST be included by senders in all requests
   - Contains the name of the receiving server

2. **Validation Requirements**:
   - The destination parameter MUST match the receiving server's name
   - If destination does not match, server MUST deny with HTTP 401 Unauthorized
   - Recipients SHOULD accept requests without this parameter (backward compatibility)

3. **Implementation**:
   - Extract destination from X-Matrix header
   - Compare against local server name  
   - Return 401 if mismatch
   - Allow missing destination for compatibility

## Security Purpose

The destination parameter ensures requests are intended for the specific receiving server, adding an extra authentication layer beyond signature verification.

## Sources

- Matrix Server-Server API v1.3: https://spec.matrix.org/v1.3/server-server-api/#request-authentication
- Matrix Server-Server API v1.3: https://spec.matrix.org/v1.3/server-server-api/#authorization-header