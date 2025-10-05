use thiserror::Error;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    Database(#[from] surrealdb::Error),

    #[error("Database error: {message} (operation: {operation})")]
    DatabaseError { message: String, operation: String },

    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Unauthorized access: {reason}")]
    Unauthorized { reason: String },

    #[error("Unauthorized {action} on {resource}: {reason}")]
    UnauthorizedAction { action: String, resource: String, reason: String },

    #[error("Forbidden: {reason}")]
    Forbidden { reason: String },

    #[error("Forbidden {action} on {resource}: {reason}")]
    ForbiddenAction { action: String, resource: String, reason: String },

    #[error("Validation error: {field}: {message}")]
    Validation { field: String, message: String },

    #[error("Validation error for {field}: {message}")]
    ValidationError { field: String, message: String },

    #[error("Conflict: {message}")]
    Conflict { message: String },

    #[error("Conflict in {field} with value {value}: {message}")]
    ConflictField { field: String, value: String, message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Serialization error: {message}")]
    SerializationError { message: String },

    #[error("Access denied: {reason}")]
    AccessDenied { reason: String },

    #[error("Invalid operation: {reason}")]
    InvalidOperation { reason: String },

    #[error("Invalid data: {message}")]
    InvalidData { message: String },

    #[error("System metrics error: {0}")]
    SystemError(String),

    #[error("State resolution failed: {0}")]
    StateResolution(String),

    #[error("External service error: {0}")]
    ExternalService(String),
}
