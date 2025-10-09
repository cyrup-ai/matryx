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
    fn test_error_debug() {
        let error = CanonicalJsonError::InvalidJson("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("InvalidJson"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_from_serde_json_error() {
        let serde_result = serde_json::from_str::<serde_json::Value>("invalid json");
        let serde_error = match serde_result {
            Err(e) => e,
            Ok(_) => panic!("Test error: Expected serde_json parse to fail for invalid JSON, but it succeeded"),
        };
        let canonical_error = CanonicalJsonError::from(serde_error);

        match canonical_error {
            CanonicalJsonError::SerializationError(_) => {
                // Successfully converted from serde_json::Error
                assert_eq!(canonical_error.to_string(), "JSON serialization failed");
            },
            _ => panic!("Expected SerializationError"),
        }
    }

    #[test]
    fn test_error_source_chain() {
        use std::error::Error;
        
        let serde_result = serde_json::from_str::<serde_json::Value>("invalid json");
        let serde_error = match serde_result {
            Err(e) => e,
            Ok(_) => panic!("Test error: Expected serde_json parse to fail for invalid JSON, but it succeeded"),
        };
        let canonical_error = CanonicalJsonError::from(serde_error);

        match canonical_error {
            CanonicalJsonError::SerializationError(_) => {
                let source = canonical_error.source();
                assert!(source.is_some(), "Error source chain should be present");
                
                if let Some(underlying) = source {
                    let error_string = underlying.to_string();
                    assert!(
                        error_string.contains("expected") || error_string.contains("invalid"),
                        "Underlying error should contain parse error details"
                    );
                }
            },
            _ => panic!("Expected SerializationError"),
        }
    }

    #[test]
    fn test_error_source_none_for_other_variants() {
        use std::error::Error;
        
        let invalid_json = CanonicalJsonError::InvalidJson("test".to_string());
        assert!(invalid_json.source().is_none());
        
        let memory = CanonicalJsonError::MemoryExhausted;
        assert!(memory.source().is_none());
        
        let recursive = CanonicalJsonError::RecursiveStructure;
        assert!(recursive.source().is_none());
    }
}
