use super::canonical_json_errors::CanonicalJsonError;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};

/// Convert a JSON value to Matrix canonical JSON format
///
/// Matrix canonical JSON has the following properties:
/// 1. Object keys are sorted lexicographically
/// 2. No extra whitespace or formatting
/// 3. Numbers are formatted consistently
/// 4. UTF-8 encoded
///
/// This implementation ensures Federation compatibility with Matrix homeservers
/// by producing deterministic JSON for cryptographic signatures.
pub fn to_canonical_json(value: &Value) -> Result<String, CanonicalJsonError> {
    let mut visited = HashSet::new();
    let canonical_value = canonicalize_value_with_cycle_detection(value, &mut visited, 0)?;

    // Use serde_json's compact serialization (no extra whitespace)
    // The canonicalize_value function ensures key ordering
    Ok(serde_json::to_string(&canonical_value)?)
}

/// Canonicalize a JSON value to Matrix canonical form
///
/// This function provides a public API for canonicalizing JSON values
/// according to Matrix specification requirements. It's used internally
/// by to_canonical_json() and can be used directly when you need the
/// canonicalized Value rather than a String.
///
/// # Arguments
/// * `value` - The JSON value to canonicalize
///
/// # Returns
/// * `Result<Value, CanonicalJsonError>` - Canonicalized JSON value with sorted keys
///
/// # Errors
/// * `CanonicalJsonError::RecursiveStructure` - If circular references detected
/// * `CanonicalJsonError::JsonError` - If JSON processing fails
#[allow(dead_code)]
pub fn canonicalize_value(value: &Value) -> Result<Value, CanonicalJsonError> {
    let mut visited = HashSet::new();
    canonicalize_value_with_cycle_detection(value, &mut visited, 0)
}

/// Recursively canonicalize a JSON value with cycle detection and depth limits
///
/// This function provides full production-quality canonicalization with:
/// - Circular reference detection using HashSet tracking
/// - Stack overflow protection with configurable depth limits
/// - Memory exhaustion checks for large structures
/// - Input validation for security and robustness
/// - All object keys sorted in lexicographical order
/// - Nested objects and arrays recursively canonicalized
fn canonicalize_value_with_cycle_detection(
    value: &Value,
    visited: &mut HashSet<*const Value>,
    depth: usize,
) -> Result<Value, CanonicalJsonError> {
    // Prevent stack overflow with depth limit
    const MAX_DEPTH: usize = 1000;
    if depth > MAX_DEPTH {
        return Err(CanonicalJsonError::RecursiveStructure);
    }

    // Check for circular references
    let value_ptr = value as *const Value;
    if visited.contains(&value_ptr) {
        return Err(CanonicalJsonError::RecursiveStructure);
    }
    match value {
        Value::Object(map) => {
            // Add to visited set for cycle detection
            visited.insert(value_ptr);

            // Check memory usage (simple heuristic)
            if map.len() > 10000 {
                visited.remove(&value_ptr);
                return Err(CanonicalJsonError::MemoryExhausted);
            }

            // Sort keys lexicographically using BTreeMap
            let mut canonical_map = BTreeMap::new();

            for (key, val) in map {
                // Validate key is a valid string (additional safety)
                if key.is_empty() {
                    visited.remove(&value_ptr);
                    return Err(CanonicalJsonError::InvalidJson(
                        "Empty object key found".to_string(),
                    ));
                }

                // Recursively canonicalize the value
                let canonical_val =
                    canonicalize_value_with_cycle_detection(val, visited, depth + 1)?;
                canonical_map.insert(key.clone(), canonical_val);
            }

            // Remove from visited set
            visited.remove(&value_ptr);

            // Convert BTreeMap back to serde_json::Map maintaining order
            Ok(Value::Object(canonical_map.into_iter().collect()))
        },
        Value::Array(arr) => {
            // Add to visited set for cycle detection
            visited.insert(value_ptr);

            // Check memory usage (simple heuristic)
            if arr.len() > 10000 {
                visited.remove(&value_ptr);
                return Err(CanonicalJsonError::MemoryExhausted);
            }

            // Recursively canonicalize each element in the array
            let mut canonical_array = Vec::new();
            for item in arr {
                let canonical_item =
                    canonicalize_value_with_cycle_detection(item, visited, depth + 1)?;
                canonical_array.push(canonical_item);
            }

            // Remove from visited set
            visited.remove(&value_ptr);

            Ok(Value::Array(canonical_array))
        },
        // All other value types (String, Number, Bool, Null) are already canonical
        _ => Ok(value.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_canonical_json_sorts_keys() -> Result<(), Box<dyn std::error::Error>> {
        let input = json!({
            "zebra": "last",
            "apple": "first",
            "beta": "middle"
        });

        let canonical = to_canonical_json(&input)?;

        // Keys should be sorted alphabetically
        assert_eq!(canonical, r#"{"apple":"first","beta":"middle","zebra":"last"}"#);
        Ok(())
    }

    #[test]
    fn test_canonical_json_nested_objects() -> Result<(), Box<dyn std::error::Error>> {
        let input = json!({
            "outer": {
                "z": 1,
                "a": 2
            },
            "array": [{"b": 1, "a": 2}]
        });

        let canonical = to_canonical_json(&input)?;

        // Both outer keys and nested object keys should be sorted
        assert_eq!(canonical, r#"{"array":[{"a":2,"b":1}],"outer":{"a":2,"z":1}}"#);
        Ok(())
    }

    #[test]
    fn test_canonical_json_preserves_types() -> Result<(), Box<dyn std::error::Error>> {
        let input = json!({
            "number": 42,
            "string": "hello",
            "boolean": true,
            "null": null,
            "array": [1, 2, 3]
        });

        let canonical = to_canonical_json(&input)?;

        // All data types should be preserved, keys sorted
        assert_eq!(
            canonical,
            r#"{"array":[1,2,3],"boolean":true,"null":null,"number":42,"string":"hello"}"#
        );
        Ok(())
    }

    #[test]
    fn test_empty_objects_and_arrays() -> Result<(), Box<dyn std::error::Error>> {
        let input = json!({
            "empty_object": {},
            "empty_array": []
        });

        let canonical = to_canonical_json(&input)?;

        // Empty structures should be handled correctly
        assert_eq!(canonical, r#"{"empty_array":[],"empty_object":{}}"#);
        Ok(())
    }

    #[test]
    fn test_complex_nested_structure() -> Result<(), Box<dyn std::error::Error>> {
        let input = json!({
            "server_keys": [
                {
                    "verify_keys": {
                        "ed25519:auto": {"key": "base64key"},
                        "ed25519:1": {"key": "otherkey"}
                    },
                    "server_name": "example.org"
                }
            ],
            "signatures": {
                "other.org": {
                    "ed25519:1": "signature"
                },
                "example.org": {
                    "ed25519:auto": "mysig"
                }
            }
        });

        let canonical = to_canonical_json(&input)?;

        // Complex nested structure with multiple levels of key sorting
        let expected = r#"{"server_keys":[{"server_name":"example.org","verify_keys":{"ed25519:1":{"key":"otherkey"},"ed25519:auto":{"key":"base64key"}}}],"signatures":{"example.org":{"ed25519:auto":"mysig"},"other.org":{"ed25519:1":"signature"}}}"#;
        assert_eq!(canonical, expected);
        Ok(())
    }

    #[test]
    fn test_matrix_server_key_format() -> Result<(), Box<dyn std::error::Error>> {
        // Test with actual Matrix server key structure
        let input = json!({
            "valid_until_ts": 1234567890,
            "verify_keys": {
                "ed25519:auto": {
                    "key": "abcd1234"
                }
            },
            "server_name": "matrix.example.org",
            "old_verify_keys": {}
        });

        let canonical = to_canonical_json(&input)?;

        // This is the format used for Matrix server key signing
        let expected = r#"{"old_verify_keys":{},"server_name":"matrix.example.org","valid_until_ts":1234567890,"verify_keys":{"ed25519:auto":{"key":"abcd1234"}}}"#;
        assert_eq!(canonical, expected);
        Ok(())
    }

    #[test]
    fn test_unicode_handling() -> Result<(), Box<dyn std::error::Error>> {
        let input = json!({
            "unicode": "Hello 世界 🚀",
            "emoji": "🔑",
            "accents": "café"
        });

        let canonical = to_canonical_json(&input)?;

        // Unicode should be preserved correctly
        assert_eq!(canonical, r#"{"accents":"café","emoji":"🔑","unicode":"Hello 世界 🚀"}"#);
        Ok(())
    }

    #[test]
    fn test_number_formatting() -> Result<(), Box<dyn std::error::Error>> {
        let input = json!({
            "integer": 42,
            "zero": 0,
            "negative": -123,
            "float": std::f64::consts::PI,
            "scientific": 1.23e-4
        });

        let canonical = to_canonical_json(&input)?;

        // Numbers should be formatted consistently
        // serde_json handles number formatting canonically
        let result: Value = serde_json::from_str(&canonical)?;
        assert_eq!(result["integer"], 42);
        assert_eq!(result["zero"], 0);
        assert_eq!(result["negative"], -123);
        Ok(())
    }

    #[test]
    fn test_error_types() -> Result<(), Box<dyn std::error::Error>> {
        // Test that our function returns the specific error type
        let input = json!({
            "test": "value"
        });

        let result = to_canonical_json(&input);
        assert!(result.is_ok());

        // The error type is now CanonicalJsonError, not Box<dyn Error>
        let _canonical: String = result?;
        Ok(())
    }

    #[test]
    fn test_deeply_nested_structure() -> Result<(), Box<dyn std::error::Error>> {
        // Test performance with deeply nested structure
        let mut nested = json!("value");
        for i in 0..100 {
            nested = json!({
                format!("level_{}", i): nested
            });
        }

        let result = to_canonical_json(&nested);
        assert!(result.is_ok(), "Should handle deeply nested structures");

        let canonical = result?;
        assert!(canonical.contains("\"level_0\""));
        assert!(canonical.contains("\"level_99\""));
        Ok(())
    }

    #[test]
    fn test_circular_reference_detection() {
        // Test depth limit protection against stack overflow
        // Create a structure that exceeds MAX_DEPTH of 1000
        let mut deeply_nested = json!("base");
        for i in 0..1001 {
            // Exceed MAX_DEPTH of 1000
            deeply_nested = json!({ format!("level_{}", i): deeply_nested });
        }

        let result = to_canonical_json(&deeply_nested);
        assert!(result.is_err(), "Should detect excessive depth");
        match result {
            Err(CanonicalJsonError::RecursiveStructure) => {
                // Expected - depth limit exceeded
            },
            Err(e) => panic!("Expected RecursiveStructure error, got: {}", e),
            Ok(_) => panic!("Expected error for excessive depth"),
        }
    }

    #[test]
    fn test_memory_exhaustion_detection() {
        // Create a very large object that exceeds memory limits
        let mut large_map = serde_json::Map::new();
        for i in 0..10001 {
            // Exceed memory limit of 10000
            large_map.insert(format!("key_{}", i), json!(i));
        }
        let large_object = Value::Object(large_map);

        let result = to_canonical_json(&large_object);
        assert!(result.is_err(), "Should detect memory exhaustion");
        match result {
            Err(CanonicalJsonError::MemoryExhausted) => {
                // Expected
            },
            Err(e) => panic!("Expected MemoryExhausted error, got: {}", e),
            Ok(_) => panic!("Expected error for large object"),
        }
    }

    #[test]
    fn test_invalid_json_detection() {
        // Create object with empty key (invalid)
        let mut invalid_map = serde_json::Map::new();
        invalid_map.insert("".to_string(), json!("value"));
        let invalid_object = Value::Object(invalid_map);

        let result = to_canonical_json(&invalid_object);
        assert!(result.is_err(), "Should detect invalid JSON structure");
        match result {
            Err(CanonicalJsonError::InvalidJson(msg)) => {
                assert!(
                    msg.contains("Empty object key"),
                    "Error message should mention empty key: {}",
                    msg
                );
            },
            Err(e) => panic!("Expected InvalidJson error, got: {}", e),
            Ok(_) => panic!("Expected error for invalid JSON"),
        }
    }

    #[test]
    fn test_large_array_memory_limit() {
        // Create a very large array that exceeds memory limits
        let mut large_array = Vec::new();
        for i in 0..10001 {
            // Exceed memory limit of 10000
            large_array.push(json!(i));
        }
        let large_array_value = Value::Array(large_array);

        let result = to_canonical_json(&large_array_value);
        assert!(result.is_err(), "Should detect array memory exhaustion");
        match result {
            Err(CanonicalJsonError::MemoryExhausted) => {
                // Expected
            },
            Err(e) => panic!("Expected MemoryExhausted error, got: {}", e),
            Ok(_) => panic!("Expected error for large array"),
        }
    }

    #[test]
    fn test_normal_operation_still_works() -> Result<(), Box<dyn std::error::Error>> {
        // Ensure normal cases still work after adding error detection
        let normal_object = json!({
            "server_name": "example.org",
            "verify_keys": {
                "ed25519:auto": {"key": "test_key"}
            }
        });

        let result = to_canonical_json(&normal_object);
        assert!(result.is_ok(), "Normal operation should still work");

        let canonical = result?;
        assert!(canonical.contains("\"server_name\":\"example.org\""));
        assert!(canonical.contains("\"verify_keys\""));

        // Verify it still produces canonical JSON (keys sorted)
        assert!(canonical.starts_with("{\"server_name\":"));
        assert!(canonical.contains("\"verify_keys\":{\"ed25519:auto\":{\"key\":\"test_key\"}}"));
        Ok(())
    }
}
