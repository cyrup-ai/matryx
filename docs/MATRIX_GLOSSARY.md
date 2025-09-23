# Matrix Protocol Glossary

## Core Concepts
- **Homeserver**: Matrix server instance that hosts users and rooms, identified by server name
- **Federation**: Server-to-server communication protocol between homeservers using HTTP APIs
- **Room**: Virtual space where users can send messages and events
- **Server Name**: Hostname and optional port identifying a Matrix homeserver
- **Well-known Discovery**: Process for discovering Matrix server endpoints via `/.well-known/matrix/server`
- **Access Token**: Opaque authentication string used by clients for API requests

## Client-Server API
- **Sync API**: Primary mechanism for clients to receive updates from the homeserver
- **Client Authentication**: Process using access tokens for secure API communication
- **JSON over HTTP**: Mandatory baseline for Matrix client-server communication
- **Content Repository**: Module for uploading and downloading media files
- **Send-to-Device**: Messaging system for ephemeral signaling between client devices

## Authentication Types (Client-Server API)
- **m.login.dummy**: No-op authentication that always succeeds, used in registration flows
- **m.login.password**: Username/password authentication
- **m.login.token**: Token-based authentication for single sign-on

## Event Types
- **PDU (Persistent Data Unit)**: Room events that are stored permanently in room history
- **EDU (Ephemeral Data Unit)**: Temporary events like typing notifications, read receipts
- **State Events**: Events that define current room state (membership, power levels, topic)
- **Timeline Events**: Events that appear in room message history
- **Account Data Events**: Per-user configuration and settings data

## Federation Protocol
- **Server-Server API**: HTTP-based protocol for homeserver communication
- **X-Matrix Authentication**: Cryptographic request signing for federation requests
- **Event Authorization**: Rules determining if events are valid in room context
- **Server Discovery**: Process for resolving server names to IP addresses and ports
- **SRV Records**: DNS records used for Matrix server discovery (`_matrix-fed._tcp`)
- **Delegation**: Mechanism for redirecting Matrix traffic to different servers

## Room Features
- **Room DAG**: Directed Acyclic Graph structure of room events
- **Room State**: Current configuration and membership of a room
- **Room Aliases**: Human-readable identifiers for rooms (e.g., `#example:matrix.org`)
- **Power Levels**: Permission system controlling user actions in rooms
- **Room Knocking**: Feature allowing users to request room membership

## Security & Encryption
- **End-to-End Encryption**: Client-side encryption ensuring message privacy
- **Device Management**: System for tracking and verifying user devices
- **Cross-Signing**: Cryptographic system for device verification
- **Key Backup**: Mechanism for securely storing encryption keys

## Testing Terminology
- **Mock Objects**: Test doubles that simulate external dependencies (legitimate testing practice)
- **Integration Tests**: Tests that verify component interactions
- **Federation Tests**: Tests that simulate server-to-server communication
- **Wiremock**: Industry standard HTTP mocking library for Rust testing