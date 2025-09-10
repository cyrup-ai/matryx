use thiserror::Error;

#[derive(Error, Debug)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    Database(#[from] surrealdb::Error),

    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Unauthorized access: {reason}")]
    Unauthorized { reason: String },

    #[error("Validation error: {field}: {message}")]
    Validation { field: String, message: String },

    #[error("Conflict: {message}")]
    Conflict { message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
