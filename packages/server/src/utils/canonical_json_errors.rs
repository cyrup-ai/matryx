use std::fmt;

/// Errors that can occur during canonical JSON processing
///
/// This provides specific error types for better debugging and error handling
/// compared to generic `Box<dyn Error>` types.
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalJsonError {
    /// Invalid JSON structure provided to canonicalization
    InvalidJson(String),
    /// Serialization failed during JSON canonicalization process
    SerializationError(String),
    /// Memory exhaustion during large object processing
    MemoryExhausted,
    /// Recursive structure detected (circular references)
    RecursiveStructure,
}

impl fmt::Display for CanonicalJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanonicalJsonError::InvalidJson(msg) => {
                write!(f, "Invalid JSON structure: {}", msg)
            },
            CanonicalJsonError::SerializationError(msg) => {
                write!(f, "JSON serialization failed: {}", msg)
            },
            CanonicalJsonError::MemoryExhausted => {
                write!(f, "Memory exhausted during canonicalization of large JSON structure")
            },
            CanonicalJsonError::RecursiveStructure => {
                write!(
                    f,
                    "Recursive JSON structure detected - cannot canonicalize circular references"
                )
            },
        }
    }
}

impl std::error::Error for CanonicalJsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // For now, these errors don't wrap other errors
        // Could be extended in the future if needed
        None
    }
}

/// Helper function to convert serde_json::Error to CanonicalJsonError
impl From<serde_json::Error> for CanonicalJsonError {
    fn from(err: serde_json::Error) -> Self {
        CanonicalJsonError::SerializationError(err.to_string())
    }
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
