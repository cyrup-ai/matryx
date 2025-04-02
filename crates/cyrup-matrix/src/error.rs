use std::fmt;
use thiserror::Error;

/// Errors that can occur when working with Matrix operations
#[derive(Error, Debug)]
pub enum Error {
    /// An error from the store module
    #[error("Store error: {0}")]
    Store(#[from] StoreError),
    
    /// An error from the client module
    #[error("Client error: {0}")]
    Client(#[from] ClientError),
    
    /// An error from the room module
    #[error("Room error: {0}")]
    Room(#[from] RoomError),
    
    /// An error from the media module
    #[error("Media error: {0}")]
    Media(#[from] MediaError),
    
    /// An error from the encryption module
    #[error("Encryption error: {0}")]
    Encryption(#[from] EncryptionError),
    
    /// An error from the sync module
    #[error("Sync error: {0}")]
    Sync(#[from] SyncError),
    
    /// An error from the notifications module
    #[error("Notification error: {0}")]
    Notification(#[from] NotificationError),

    /// An error from the database module
    #[error("Database error: {0}")]
    Database(String),
}

/// Result type for Matrix operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when working with Matrix storage operations
#[derive(Error, Debug)]
pub enum StoreError {
    /// An error from the underlying Matrix SDK
    #[error("Matrix SDK error: {0}")]
    MatrixSdk(String),

    /// A communication error with the storage backend
    #[error("Storage communication error: {0}")]
    StorageCommunication(String),

    /// An error occurred while spawning a task
    #[error("Task spawn error: {0}")]
    TaskSpawn(String),

    /// An error during serialization/deserialization
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// An error with the storage format or schema
    #[error("Storage format error: {0}")]
    StorageFormat(String),
}

impl StoreError {
    /// Create a new MatrixSdk error
    pub fn matrix_sdk<E: fmt::Display>(error: E) -> Self {
        Self::MatrixSdk(error.to_string())
    }
}

impl From<matrix_sdk_base::store::StoreError> for StoreError {
    fn from(error: matrix_sdk_base::store::StoreError) -> Self {
        Self::MatrixSdk(error.to_string())
    }
}

// Add conversion from db::Error to Error
impl From<crate::db::Error> for Error {
    fn from(error: crate::db::Error) -> Self {
        Self::Database(error.to_string())
    }
}

/// Errors that can occur when working with Matrix client operations
#[derive(Error, Debug)]
pub enum ClientError {
    /// An error from the underlying Matrix SDK
    #[error("Matrix SDK error: {0}")]
    MatrixSdk(String),
    
    /// A room was not found
    #[error("Room not found: {0}")]
    RoomNotFound(String),
    
    /// An invalid parameter was provided
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    
    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    /// Encryption error
    #[error("Encryption error: {0}")]
    Encryption(String),
    
    /// Network error
    #[error("Network error: {0}")]
    Network(String),
    
    /// Rate limit error
    #[error("Rate limit error: {0}")]
    RateLimit(String),
    
    /// Server error
    #[error("Server error: {0}")]
    Server(String),
}

impl ClientError {
    /// Create a new MatrixSdk error
    pub fn matrix_sdk<E: fmt::Display>(error: E) -> Self {
        Self::MatrixSdk(error.to_string())
    }
}

/// Errors that can occur when working with Matrix room operations
#[derive(Error, Debug)]
pub enum RoomError {
    /// An error from the underlying Matrix SDK
    #[error("Matrix SDK error: {0}")]
    MatrixSdk(String),
    
    /// An invalid parameter was provided
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    
    /// The room is not joined
    #[error("Not joined to room")]
    NotJoined,
    
    /// The user doesn't have permission for this operation
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    /// The message is too large
    #[error("Message too large")]
    MessageTooLarge,
    
    /// The event was not found
    #[error("Event not found: {0}")]
    EventNotFound(String),
}

impl RoomError {
    /// Create a new MatrixSdk error
    pub fn matrix_sdk<E: fmt::Display>(error: E) -> Self {
        Self::MatrixSdk(error.to_string())
    }
}

/// Errors that can occur when working with Matrix media operations
#[derive(Error, Debug)]
pub enum MediaError {
    /// An error from the underlying Matrix SDK
    #[error("Matrix SDK error: {0}")]
    MatrixSdk(String),
    
    /// An invalid media URI
    #[error("Invalid media URI: {0}")]
    InvalidUri(String),
    
    /// File I/O error
    #[error("File I/O error: {0}")]
    IoError(String),
    
    /// Media is too large
    #[error("Media too large")]
    MediaTooLarge,
    
    /// Unsupported media type
    #[error("Unsupported media type: {0}")]
    UnsupportedMediaType(String),
}

impl MediaError {
    /// Create a new MatrixSdk error
    pub fn matrix_sdk<E: fmt::Display>(error: E) -> Self {
        Self::MatrixSdk(error.to_string())
    }
}

/// Errors that can occur when working with Matrix encryption operations
#[derive(Error, Debug)]
pub enum EncryptionError {
    /// An error from the underlying Matrix SDK
    #[error("Matrix SDK error: {0}")]
    MatrixSdk(String),
    
    /// Invalid verification type for the requested operation
    #[error("Invalid verification type: {0}")]
    InvalidVerificationType(String),
    
    /// This verification type is not supported by this wrapper
    #[error("Unsupported verification type: {0}")]
    UnsupportedVerificationType(String),
    
    /// The verification request has expired
    #[error("Verification request expired")]
    VerificationExpired,
    
    /// The verification request was canceled
    #[error("Verification request canceled: {0}")]
    VerificationCanceled(String),
    
    /// The recovery key is invalid
    #[error("Invalid recovery key: {0}")]
    InvalidRecoveryKey(String),
    
    /// Missing encryption keys
    #[error("Missing encryption keys")]
    MissingKeys,
}

impl EncryptionError {
    /// Create a new MatrixSdk error
    pub fn matrix_sdk<E: fmt::Display>(error: E) -> Self {
        Self::MatrixSdk(error.to_string())
    }
}

/// Errors that can occur when working with Matrix sync operations
#[derive(Error, Debug)]
pub enum SyncError {
    /// An error from the underlying Matrix SDK
    #[error("Matrix SDK error: {0}")]
    MatrixSdk(String),
    
    /// Network error during sync
    #[error("Network error during sync: {0}")]
    NetworkError(String),
    
    /// Sync timeout
    #[error("Sync timeout")]
    Timeout,
    
    /// Error in filter definition
    #[error("Filter error: {0}")]
    FilterError(String),
}

impl SyncError {
    /// Create a new MatrixSdk error
    pub fn matrix_sdk<E: fmt::Display>(error: E) -> Self {
        Self::MatrixSdk(error.to_string())
    }
}

/// Errors that can occur when working with Matrix notification settings
#[derive(Error, Debug)]
pub enum NotificationError {
    /// An error from the underlying Matrix SDK
    #[error("Matrix SDK error: {0}")]
    MatrixSdk(String),
    
    /// Invalid notification rule
    #[error("Invalid notification rule: {0}")]
    InvalidRule(String),
    
    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

impl NotificationError {
    /// Create a new MatrixSdk error
    pub fn matrix_sdk<E: fmt::Display>(error: E) -> Self {
        Self::MatrixSdk(error.to_string())
    }
}