# Database Functionality in Cyrum

## SQLite Usage

Cyrum uses SQLite as a storage backend for the Matrix SDK. The application doesn't directly interact with SQLite through custom queries but rather relies on the Matrix SDK's abstraction layer to handle database operations.

## Key Database Functionality

1. **Session Storage**:
   - Matrix client sessions are stored in SQLite database files
   - Located in the user's profile directory under the `sqlite_dir` path
   - Sessions contain authentication tokens, device information, and user preferences

2. **Encryption Key Management**:
   - Room encryption keys are stored in the SQLite database
   - Supports export and import of keys for backup and recovery
   - Implements a migration system from older Sled storage to SQLite

3. **Message History**:
   - Stores message timelines and room state
   - Maintains read receipts and typing notifications
   - Preserves reactions and edited messages

4. **Room Management**:
   - Stores room membership information
   - Tracks room display names and aliases
   - Preserves space hierarchy relationships

5. **User Data**:
   - Maintains user display names and avatars
   - Tracks presence status
   - Stores verification status of users and devices

## Migration from Sled to SQLite

The application includes a specialized migration system to transition from the older Sled database (used in earlier versions) to SQLite:

1. **Detection**: Checks if a Sled database exists but SQLite doesn't, triggering migration
2. **Export**: Extracts encryption keys from the Sled store
3. **Encryption**: Secures the exported keys with a randomly generated passphrase
4. **Import**: After setting up the new SQLite database, imports the encrypted keys

## Database File Locations

The application maintains clear separation between database types:

- **SQLite database**: Stored in the user's profile directory under a dedicated SQLite folder
- **Legacy Sled database**: Located in a separate directory for backward compatibility

## Data Management

The application handles several database operations:

1. **Login and Authentication**:
   - Stores session tokens in the SQLite database
   - Supports session restoration across application restarts
   - Implements proper logout by clearing the session data

2. **Message Synchronization**:
   - Fetches and persists new messages in the background
   - Implements pagination for loading older messages
   - Uses a batched loading system to efficiently retrieve messages when needed

3. **Room State Management**:
   - Persists room membership changes
   - Tracks room settings and properties
   - Maintains space hierarchies and relationships

4. **Encryption**:
   - Stores encryption keys for secure message decryption
   - Manages verification of other users and devices
   - Supports secure key backup and restoration

The database implementation is designed to be resilient, with proper error handling and recovery mechanisms. The migration system ensures users can seamlessly transition from older versions without losing critical encryption keys.