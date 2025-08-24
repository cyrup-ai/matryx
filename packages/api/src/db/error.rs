use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::io::Error as IoError;
use surrealdb::error::Api as SurrealDbApiError;
use surrealdb::Error as SurrealDbError;
use thiserror::Error;

/// Result type for database operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error type for database operations
#[derive(Error, Debug)]
pub enum Error {
    /// Database error
    #[error("Database error: {0}")]
    Database(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Validation error
    #[error("Validation error: {0}")]
    Validation(String),

    /// Not found error
    #[error("Not found: {0}")]
    NotFound(ErrorContext),

    /// Already exists error
    #[error("Already exists: {0}")]
    AlreadyExists(ErrorContext),

    /// Migration error
    #[error("Migration error: {0}")]
    Migration(ErrorContext),

    /// Other error
    #[error("Error: {0}")]
    Other(String),

    /// Error variant for the `timeout` error
    #[error("Timeout error: {0}")]
    Timeout(String),
}

impl Error {
    /// Create a new database error
    pub fn database(message: impl ToString) -> Self {
        Self::Database(message.to_string())
    }

    /// Create a new serialization error
    pub fn serialization(message: impl ToString) -> Self {
        Self::Serialization(message.to_string())
    }

    /// Create a new validation error
    pub fn validation(message: impl ToString) -> Self {
        Self::Validation(message.to_string())
    }

    /// Create a new not found error
    pub fn not_found(message: impl ToString) -> Self {
        Self::NotFound(ErrorContext::new(message))
    }

    /// Create a new already exists error
    pub fn already_exists(message: impl ToString) -> Self {
        Self::AlreadyExists(ErrorContext::new(message))
    }

    /// Create a new migration error
    pub fn migration(context: ErrorContext) -> Self {
        Self::Migration(context)
    }

    /// Create a new other error
    pub fn other(message: impl ToString) -> Self {
        Self::Other(message.to_string())
    }

    /// Create a new timeout error
    pub fn timeout<S: Into<String>>(message: S) -> Self {
        Self::Timeout(message.into())
    }

    /// Convert to error context
    pub fn to_context(self) -> ErrorContext {
        match self {
            Self::NotFound(ctx) => ctx,
            Self::AlreadyExists(ctx) => ctx,
            Self::Migration(ctx) => ctx,
            _ => ErrorContext::new(self.to_string()),
        }
    }
}

/// Convert from SurrealDB API error
impl From<SurrealDbApiError> for Error {
    fn from(error: SurrealDbApiError) -> Self {
        match error {
            SurrealDbApiError::Query(e) => Self::Database(format!("Query error: {}", e)),
            _ => Self::Database(format!("SurrealDB API error: {}", error)),
        }
    }
}

/// Convert from SurrealDB error
impl From<SurrealDbError> for Error {
    fn from(error: SurrealDbError) -> Self {
        Self::Database(format!("SurrealDB error: {}", error))
    }
}

/// Convert from std::io::Error
impl From<IoError> for Error {
    fn from(error: IoError) -> Self {
        Self::Other(format!("IO error: {}", error))
    }
}

/// Convert from serde_json error
impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(format!("JSON error: {}", error))
    }
}

/// Error context for richer error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Error message
    pub message: String,
    /// Error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// Entity ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Entity type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<String>,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(message: impl ToString) -> Self {
        Self {
            message: message.to_string(),
            details: None,
            id: None,
            entity_type: None,
        }
    }

    /// Add details to the error context
    pub fn with_details(mut self, details: impl ToString) -> Self {
        self.details = Some(details.to_string());
        self
    }

    /// Add entity ID to the error context
    pub fn with_id(mut self, id: impl ToString) -> Self {
        self.id = Some(id.to_string());
        self
    }

    /// Add entity type to the error context
    pub fn with_entity_type(mut self, entity_type: impl ToString) -> Self {
        self.entity_type = Some(entity_type.to_string());
        self
    }
}

impl Display for ErrorContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(details) = &self.details {
            write!(f, " ({})", details)?;
        }
        if let Some(id) = &self.id {
            if let Some(entity_type) = &self.entity_type {
                write!(f, " [{}:{}]", entity_type, id)?;
            } else {
                write!(f, " [{}]", id)?;
            }
        } else if let Some(entity_type) = &self.entity_type {
            write!(f, " [{}]", entity_type)?;
        }
        Ok(())
    }
}
