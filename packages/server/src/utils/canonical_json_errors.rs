use std::fmt;
use thiserror::Error;

/// Errors that can occur during canonical JSON processing
///
/// This provides specific error types for better debugging and error handling
/// compared to generic `Box<dyn Error>` types.
#[derive(Error, Debug)]
pub enum CanonicalJsonError {
    /// Invalid JSON structure provided to canonicalization
    #[error("Invalid JSON structure: {0}")]
    InvalidJson(String),
    
    /// Serialization failed during JSON canonicalization process
    #[error("JSON serialization failed")]
    SerializationError(#[from] serde_json::Error),
    
    /// Memory exhaustion during large object processing
    #[error("Memory exhausted during canonicalization of large JSON structure")]
    MemoryExhausted,
    
    /// Recursive structure detected (circular references)
    #[error("Recursive JSON structure detected - cannot canonicalize circular references")]
    RecursiveStructure,
}




#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let invalid_json_error = CanonicalJsonError::InvalidJson("missing quotes".to_string());
        assert_eq!(invalid_json_error.to_string(), "Invalid JSON structure: missing quotes");

        let serialization_error =
            CanonicalJsonError::SerializationError("buffer overflow".to_string());
        assert_eq!(serialization_error.to_string(), "JSON serialization failed: buffer overflow");

        let memory_error = CanonicalJsonError::MemoryExhausted;
        assert_eq!(
            memory_error.to_string(),
            "Memory exhausted during canonicalization of large JSON structure"
        );

        let recursive_error = CanonicalJsonError::RecursiveStructure;
        assert_eq!(
            recursive_error.to_string(),
            "Recursive JSON structure detected - cannot canonicalize circular references"
        );
    }

    #[test]
    fn test_error_clone_and_equality() {
        let error1 = CanonicalJsonError::InvalidJson("test".to_string());
        let error2 = error1.clone();
        assert_eq!(error1, error2);
    }

    #[test]
    fn test_from_serde_json_error() {
        let serde_error = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let canonical_error = CanonicalJsonError::from(serde_error);

        match canonical_error {
            CanonicalJsonError::SerializationError(msg) => {
                assert!(msg.contains("expected") || msg.contains("invalid"));
            },
            _ => panic!("Expected SerializationError"),
        }
    }
}
