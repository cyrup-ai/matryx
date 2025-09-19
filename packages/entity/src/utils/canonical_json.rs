use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Converts a JSON value to canonical JSON format according to Matrix specification
///
/// Matrix canonical JSON format requirements:
/// - Object keys MUST be sorted lexicographically
/// - No unnecessary whitespace (compact representation)
/// - UTF-8 encoded
/// - No trailing commas
/// - Boolean, null, and numeric values in lowercase
/// - String values properly escaped
///
/// This is used for cryptographic signature verification where the exact
/// byte representation of JSON is critical for reproducible results.
///
/// # Arguments
/// * `value` - The JSON value to canonicalize
///
/// # Returns
/// * `Result<String, CanonicalJsonError>` - The canonical JSON string or an error
///
/// # Examples
/// ```rust
/// use serde_json::json;
/// use matryx_entity::utils::canonical_json;
///
/// let data = json!({
///     "z_key": "last",
///     "a_key": "first",
///     "numeric": 42,
///     "boolean": true,
///     "null_value": null
/// });
///
/// let canonical = canonical_json(&data)?;
/// // Result: {"a_key":"first","boolean":true,"null_value":null,"numeric":42,"z_key":"last"}
/// ```
pub fn canonical_json(value: &Value) -> Result<String, CanonicalJsonError> {
    let canonical_value = canonicalize_value(value)?;
    serde_json::to_string(&canonical_value)
        .map_err(|e| CanonicalJsonError::SerializationError(e.to_string()))
}

/// Custom error type for canonical JSON operations
#[derive(Debug, thiserror::Error)]
pub enum CanonicalJsonError {
    #[error("JSON serialization failed: {0}")]
    SerializationError(String),

    #[error("Invalid JSON structure: {0}")]
    InvalidStructure(String),

    #[error("Unsupported JSON type: {0}")]
    UnsupportedType(String),
}

/// Internal function to recursively canonicalize JSON values
///
/// Handles all JSON value types and ensures proper ordering and formatting:
/// - Objects: Sort keys lexicographically, recursively canonicalize values
/// - Arrays: Preserve order, recursively canonicalize elements  
/// - Strings: Keep as-is (proper escaping handled by serde_json)
/// - Numbers: Keep as-is (canonical representation handled by serde_json)
/// - Booleans: Keep as-is (lowercase true/false handled by serde_json)
/// - Null: Keep as-is (lowercase null handled by serde_json)
fn canonicalize_value(value: &Value) -> Result<Value, CanonicalJsonError> {
    match value {
        Value::Object(obj) => {
            // Sort object keys lexicographically using BTreeMap
            let mut canonical_obj = BTreeMap::new();

            for (key, val) in obj {
                let canonical_val = canonicalize_value(val)?;
                canonical_obj.insert(key.clone(), canonical_val);
            }

            // Convert BTreeMap back to serde_json::Map for serialization
            let mut map = Map::new();
            for (key, val) in canonical_obj {
                map.insert(key, val);
            }

            Ok(Value::Object(map))
        },
        Value::Array(arr) => {
            // Preserve array order but canonicalize elements
            let mut canonical_arr = Vec::new();
            for item in arr {
                canonical_arr.push(canonicalize_value(item)?);
            }
            Ok(Value::Array(canonical_arr))
        },
        // Primitive types are already in canonical form
        Value::String(_) | Value::Number(_) | Value::Bool(_) | Value::Null => Ok(value.clone()),
    }
}

/// Canonicalize and sign a JSON object for Matrix event signatures
///
/// Specialized function for Matrix event signing that:
/// - Excludes the "signatures" and "unsigned" fields from canonicalization
/// - Applies Matrix canonical JSON rules for reproducible signatures
/// - Returns the exact bytes that should be signed
///
/// # Arguments
/// * `event` - The Matrix event object to canonicalize for signing
///
/// # Returns
/// * `Result<String, CanonicalJsonError>` - Canonical JSON without signatures
pub fn canonical_json_for_signing(event: &Value) -> Result<String, CanonicalJsonError> {
    let mut signing_event = event.clone();

    // Remove fields that should not be included in signature calculation
    if let Some(obj) = signing_event.as_object_mut() {
        obj.remove("signatures");
        obj.remove("unsigned");
    }

    canonical_json(&signing_event)
}

/// Verify that a JSON string is in canonical format
///
/// Utility function to validate that a JSON string follows Matrix canonical rules.
/// Useful for debugging and testing signature verification issues.
///
/// # Arguments
/// * `json_str` - The JSON string to validate
///
/// # Returns  
/// * `Result<bool, CanonicalJsonError>` - True if canonical, false otherwise
pub fn is_canonical_json(json_str: &str) -> Result<bool, CanonicalJsonError> {
    let value: Value = serde_json::from_str(json_str)
        .map_err(|e| CanonicalJsonError::SerializationError(e.to_string()))?;

    let canonical = canonical_json(&value)?;
    Ok(canonical == json_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_canonical_json_object_key_sorting() {
        let data = json!({
            "z_last": "value_z",
            "a_first": "value_a",
            "m_middle": "value_m"
        });

        let result = canonical_json(&data).unwrap();
        assert_eq!(result, r#"{"a_first":"value_a","m_middle":"value_m","z_last":"value_z"}"#);
    }

    #[test]
    fn test_canonical_json_nested_objects() {
        let data = json!({
            "outer": {
                "z_nested": 123,
                "a_nested": true
            },
            "a_key": "value"
        });

        let result = canonical_json(&data).unwrap();
        assert_eq!(result, r#"{"a_key":"value","outer":{"a_nested":true,"z_nested":123}}"#);
    }

    #[test]
    fn test_canonical_json_arrays_preserve_order() {
        let data = json!({
            "array": ["z", "a", "m"],
            "numbers": [3, 1, 2]
        });

        let result = canonical_json(&data).unwrap();
        assert_eq!(result, r#"{"array":["z","a","m"],"numbers":[3,1,2]}"#);
    }

    #[test]
    fn test_canonical_json_primitive_types() {
        let data = json!({
            "string": "test",
            "number": 42,
            "boolean": true,
            "null_value": null
        });

        let result = canonical_json(&data).unwrap();
        assert_eq!(result, r#"{"boolean":true,"null_value":null,"number":42,"string":"test"}"#);
    }

    #[test]
    fn test_canonical_json_for_signing_removes_signatures() {
        let event = json!({
            "event_id": "$test:example.com",
            "content": {"body": "Hello"},
            "signatures": {
                "example.com": {
                    "ed25519:key": "signature_data"
                }
            },
            "unsigned": {
                "age": 1234
            }
        });

        let result = canonical_json_for_signing(&event).unwrap();
        assert!(!result.contains("signatures"));
        assert!(!result.contains("unsigned"));
        assert!(result.contains("event_id"));
        assert!(result.contains("content"));
    }

    #[test]
    fn test_is_canonical_json_validation() {
        let canonical = r#"{"a":"first","z":"last"}"#;
        let non_canonical = r#"{"z":"last","a":"first"}"#;

        assert!(is_canonical_json(canonical).unwrap());
        assert!(!is_canonical_json(non_canonical).unwrap());
    }

    #[test]
    fn test_canonical_json_empty_object() {
        let data = json!({});
        let result = canonical_json(&data).unwrap();
        assert_eq!(result, "{}");
    }

    #[test]
    fn test_canonical_json_empty_array() {
        let data = json!([]);
        let result = canonical_json(&data).unwrap();
        assert_eq!(result, "[]");
    }

    #[test]
    fn test_canonical_json_string_escaping() {
        let data = json!({
            "escaped": "line1\nline2\ttab\"quote\\backslash"
        });

        let result = canonical_json(&data).unwrap();
        // Verify proper JSON escaping is preserved
        assert!(result.contains(r#""escaped":"line1\nline2\ttab\"quote\\backslash""#));
    }

    #[test]
    fn test_canonical_json_unicode_handling() {
        let data = json!({
            "unicode": "Hello 👋 World 🌍",
            "émoji": "café"
        });

        let result = canonical_json(&data).unwrap();
        // Should preserve Unicode characters properly
        assert!(result.contains("👋"));
        assert!(result.contains("🌍"));
        assert!(result.contains("café"));
    }
}
