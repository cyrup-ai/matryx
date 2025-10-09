# Matrix OAuth2 Scopes (MSC2967)

**Source:** https://github.com/matrix-org/matrix-spec-proposals/blob/main/proposals/2967-api-scopes.md

## Defined Matrix OAuth2 Scopes

### 1. Full API Access
```
urn:matrix:client:api:*
```
- Grants **full access** to the Client-Server API
- Wildcard scope for complete API access
- Most commonly used scope for Matrix clients

### 2. Device-Specific Access
```
urn:matrix:client:device:<device_id>
```
- Allows associating a specific device ID with an OAuth grant
- Pattern: `urn:matrix:client:device:` followed by the device ID
- Example: `urn:matrix:client:device:ABCDEFG`

## Future Scope Patterns (Not Yet Implemented)

The MSC proposes future granular scopes:
- `urn:matrix:client:api:<permission>` - Permission-based (read, write, delete, append)
- `urn:matrix:client:api:<permission>:*` - Wildcard permission scopes
- `urn:matrix:client:api:read:<resource>` - Resource-specific read access
  - Example: `urn:matrix:client:api:read:#matrix-auth`

## Unstable Prefix (During Development)
```
urn:matrix:org.matrix.msc2967.client
```
- Use unstable prefix during MSC development
- Switch to stable prefix after MSC acceptance

## Scope Validation Strategy

### Exact Match
- `urn:matrix:client:api:*` - Check for exact string match

### Pattern Match (Device Scopes)
- `urn:matrix:client:device:DEVICEID` - Validate prefix and extract device ID
- Validate that device ID exists and belongs to the user

### Wildcard Support
- If client has `urn:matrix:client:api:*` in allowed_scopes
- Then any `urn:matrix:client:api:*` request is valid
- Device scopes require exact device ID match or pattern validation

## Implementation for MaxTryX

```rust
// Recommended allowed_scopes for Matrix clients:
vec![
    "openid".to_string(),
    "profile".to_string(),
    "email".to_string(),
    "urn:matrix:client:api:*".to_string(),
]
```
