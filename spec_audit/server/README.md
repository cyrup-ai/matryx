# Matrix Server-to-Server API Specification

This directory contains the Matrix Server-to-Server API specification decomposed into logical sections for easier navigation and implementation.

## Structure

### Core Foundation
- [**01-introduction.md**](01-introduction.md) - Overview, API standards, TLS requirements
- [**02-server-discovery.md**](02-server-discovery.md) - Server name resolution and well-known delegation  
- [**03-server-keys.md**](03-server-keys.md) - Key publishing, retrieval, and notary servers
- [**04-authentication.md**](04-authentication.md) - Request/response authentication and TLS certificates

### Event Processing
- [**05-transactions.md**](05-transactions.md) - Transaction format and limits
- [**06-pdus.md**](06-pdus.md) - Persistent Data Units, validation, and authorization
- [**07-edus.md**](07-edus.md) - Ephemeral Data Units and event types
- [**08-room-state.md**](08-room-state.md) - State resolution algorithms

### Federation Operations  
- [**09-backfill.md**](09-backfill.md) - Event history retrieval and missing events
- [**10-event-retrieval.md**](10-event-retrieval.md) - Individual event and state queries
- [**11-room-joining.md**](11-room-joining.md) - Join handshake and restricted rooms
- [**12-room-knocking.md**](12-room-knocking.md) - Knock requests and responses
- [**13-room-invites.md**](13-room-invites.md) - Invitation process and third-party invites
- [**14-room-leaving.md**](14-room-leaving.md) - Leave process and invite rejection

### Specialized Features
- [**15-room-directory.md**](15-room-directory.md) - Published room discovery
- [**16-spaces.md**](16-spaces.md) - Space hierarchies and room relationships
- [**17-typing.md**](17-typing.md) - Typing notification EDUs
- [**18-presence.md**](18-presence.md) - Presence update EDUs  
- [**19-receipts.md**](19-receipts.md) - Read receipt EDUs
- [**20-queries.md**](20-queries.md) - Directory and profile queries

### Advanced Features
- [**21-openid.md**](21-openid.md) - OpenID token exchange
- [**22-device-management.md**](22-device-management.md) - Device list synchronization
- [**23-end-to-end-encryption.md**](23-end-to-end-encryption.md) - Key claiming and device queries
- [**24-send-to-device.md**](24-send-to-device.md) - Direct device messaging
- [**25-content-repository.md**](25-content-repository.md) - Media download and thumbnails

### Security & Implementation
- [**26-server-acls.md**](26-server-acls.md) - Server access control lists
- [**27-event-signing.md**](27-event-signing.md) - Digital signatures and hash verification
- [**28-security.md**](28-security.md) - Security considerations

## Implementation Notes

Each section is designed to be:
- **Self-contained** - Can be implemented independently with clear dependencies
- **API-focused** - Organized around endpoint functionality rather than concepts
- **Implementation-ready** - Contains complete endpoint specifications with examples

## Quick Reference

- **Core endpoints**: Sections 1-8 (foundation and basic event processing)
- **Room operations**: Sections 11-14 (joining, knocking, invites, leaving)
- **Federation queries**: Sections 9-10, 20 (data retrieval and queries)
- **Real-time features**: Sections 17-19 (typing, presence, receipts)
- **Encryption support**: Sections 22-24 (devices, E2E, messaging)

The original complete specification is retained at the parent directory level as `MATRIX_SERVER_SPEC.md`.