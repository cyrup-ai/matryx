# Matrix OAuth 2.0 Scopes (MSC2967)

**Source:** https://github.com/matrix-org/matrix-spec-proposals/blob/main/proposals/2967-api-scopes.md

## Defined Matrix Scopes

### 1. Full API Access
- **Scope:** `urn:matrix:client:api:*`
- **Description:** Grants full access to the Client-Server API
- **Use Case:** Traditional Matrix clients that need complete API access

### 2. Device ID Scope
- **Scope:** `urn:matrix:client:device:<device_id>`
- **Description:** Associates a specific device ID with an OAuth grant
- **Use Case:** Device-specific authentication and authorization

## Future Scope Framework (Potential)

The MSC proposes a framework for future granular scopes:

### Permission-Based Scopes
- `urn:matrix:client:api:<permission>` or `urn:matrix:client:api:<permission>:*`
- Potential permissions: read, write, delete, append

### Resource-Based Scopes
- `urn:matrix:client:api:read:<resource>`
- Example: `urn:matrix:client:api:read:#matrix-auth`

## Unstable Prefix

During development, use unstable prefix:
- `urn:matrix:org.matrix.msc2967.client`

## Implementation Notes for MaxTryX

1. Support `urn:matrix:client:api:*` as primary Matrix scope
2. Support device-specific scopes with pattern matching
3. Prepare for future granular scopes by supporting pattern matching
4. Use wildcard matching for scopes ending in `*`
