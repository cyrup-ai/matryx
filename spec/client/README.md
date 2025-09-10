# Matrix Client-Server API Specification

This directory contains the Matrix Client-Server API specification decomposed into logical sections for improved maintainability and development workflow.

## Overview

The original [`MATRIX_CLIENT_SPEC.md`](../MATRIX_CLIENT_SPEC.md) (30,281 lines) has been broken down into **7 separate files**, each covering distinct functional areas of the Matrix Client-Server API. This decomposition ensures that:

- **Sum of all parts equals the whole**: All content is preserved verbatim
- **Logical coherence**: Related functionality stays together
- **Development efficiency**: Teams can work on specific areas without conflicts
- **Manageable file sizes**: Each file is appropriately sized for editing and review

## File Structure

### 1. [Foundation API](./01_foundation_api.md) (~5,650 lines)
**Core Matrix client-server API foundations**

- **API Standards**: JSON/HTTP communication standards, error handling, rate limiting
- **Web Browser Clients**: Browser-specific requirements and CORS
- **Server Discovery**: Well-known URIs and homeserver auto-discovery
- **Client Authentication**: Complete authentication system including:
  - Legacy API (User-Interactive Authentication)
  - OAuth 2.0 API (authorization code flow)
  - Access token management and refresh
  - Soft logout and device management
- **Capabilities Negotiation**: Feature detection and server capabilities
- **Filtering**: Event filtering for sync operations and lazy-loading
- **Events**: Event types, format specifications, and size limits
- **Basic Sync API**: Real-time synchronization foundations

### 2. [Rooms & Users](./02_rooms_users.md) (~6,010 lines)
**Room and user management**

- **Advanced Sync API**: Complete sync implementation with timeline management
- **Room Creation**: Room creation, configuration, and initial state
- **Room Management**: Membership, permissions, aliases, and directory operations
- **Room Discovery**: Public room directory and search
- **User Data**: User directory, profiles, and account information
- **Module Framework**: Feature profiles and client classifications

### 3. [Messaging & Communication](./03_messaging_communication.md) (~4,358 lines)
**Real-time communication features**

- **Instant Messaging**: Message types, formatting, and room names/topics
- **Rich Replies**: Message threading and reply mechanisms
- **Voice over IP**: WebRTC calling infrastructure and signaling
- **Typing Notifications**: Real-time typing indicators
- **Receipts**: Read receipt tracking and delivery confirmations
- **Read/Unread Markers**: Message read state management
- **Presence**: User online/offline status and availability
- **Content Repository**: Media upload/download APIs and management

### 4. [Security & Encryption](./04_security_encryption.md) (~5,432 lines)
**Security and encryption**

- **Send-to-Device Messaging**: Direct device communication
- **Device Management**: Device registration, verification, and control
- **End-to-End Encryption**: Complete E2EE system including:
  - Key management and distribution
  - Cross-signing and device verification
  - Key backup and recovery
  - Megolm and Olm protocols
- **Secrets**: Secure key storage and sharing mechanisms

### 5. [Advanced Features](./05_advanced_features.md) (~4,276 lines)
**Advanced room and user features**

- **Room History Visibility**: Access control for message history
- **Push Notifications**: Real-time notification system and configuration
- **Third-party Invites**: External user invitation system
- **Server Side Search**: Full-text search capabilities
- **Guest Access**: Anonymous room access and limitations
- **Room Previews**: Room content preview for non-members
- **Room Tagging**: Custom room organization and categorization

### 6. [User Experience](./06_user_experience.md) (~3,149 lines)
**User experience and integration**

- **Client Config**: User preferences and settings synchronization
- **Server Administration**: Administrative endpoints and management
- **Event Context**: Message context retrieval around specific events
- **SSO Authentication**: Single sign-on integration
- **Direct Messaging**: 1:1 conversation management
- **Ignoring Users**: User blocking functionality
- **Sticker Messages**: Custom emoji/sticker support
- **Content Reporting**: Abuse reporting system
- **Third-party Networks**: Bridge integrations
- **OpenID**: Identity provider integration
- **Server ACLs**: Server access control lists
- **User/Room Mentions**: @mention functionality
- **Room Upgrades**: Room version migrations
- **Server Notices**: System notifications
- **Moderation Policy Lists**: Shared moderation rules

### 7. [Relationship Features](./07_relationship_features.md) (~1,543 lines)
**Event relationships and advanced UX**

- **Spaces**: Hierarchical room organization system
- **Event Replacements**: Message editing and content updates
- **Event Annotations/Reactions**: Message reactions and emoji responses
- **Threading**: Threaded conversations and reply chains
- **Reference Relations**: Event relationship system and references

## API Standards Compliance

All files maintain the same API standards as defined in the original specification:

- **HTTP/JSON Communication**: RESTful endpoints with JSON payloads
- **Authentication**: Bearer token authentication and OAuth 2.0 support
- **Error Handling**: Standardized error responses with proper HTTP status codes
- **Rate Limiting**: Consistent rate limiting across all endpoints
- **Versioning**: API version negotiation and feature discovery
- **Transaction IDs**: Idempotent request handling

## Development Guidelines

### Working with Decomposed Files

1. **API Dependencies**: Earlier files contain dependencies for later files
2. **Cross-References**: Links between files use relative paths where possible
3. **Consistency**: All files follow the same formatting and documentation standards
4. **Completeness**: Each file is self-contained for its functional domain

### Making Changes

When modifying the specification:

1. **Identify the correct file**: Use this index to locate the appropriate section
2. **Maintain consistency**: Follow existing patterns within the file
3. **Update cross-references**: Ensure links between files remain valid  
4. **Test completeness**: Verify that changes don't break the "sum equals whole" principle

### Integration Points

Key integration points between files:

- **Authentication** (Foundation → All): Access token usage throughout
- **Events** (Foundation → All): Event format specifications
- **Rooms** (Rooms/Users → All): Room context for most features
- **Sync** (Foundation → Messaging): Real-time updates
- **Encryption** (Security → Messaging): Secure message delivery

## Version Information

- **Source**: [Matrix Client-Server API Specification](https://spec.matrix.org/unstable/client-server-api/)
- **Version**: Unstable (as of decomposition date)
- **Total Lines**: 30,281 lines across 7 files
- **Decomposition Date**: [Current Date]

## Maintenance Notes

This decomposition maintains:
- ✅ **Complete API Coverage**: All endpoints and functionality preserved
- ✅ **Exact Content**: No modifications to API definitions or behavior
- ✅ **Proper Cross-References**: Internal links maintained and updated
- ✅ **Consistent Formatting**: Uniform markdown structure across files
- ✅ **Development Workflow**: Logical file boundaries for team collaboration

The original `MATRIX_CLIENT_SPEC.md` file is retained as the authoritative source and should be considered the definitive reference in case of any discrepancies.