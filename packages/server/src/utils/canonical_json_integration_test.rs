#[cfg(test)]
mod integration_tests {
    use super::canonical_json::to_canonical_json;
    use serde_json::{json, Value};

    /// Test canonical JSON with actual Matrix server key structure
    /// This verifies that server key signing will work correctly with matrix.org
    #[test]
    fn test_matrix_server_key_canonical_json() {
        // This is the exact structure used in server.rs:build_canonical_server_json()
        let server_object = json!({
            "server_name": "example.homeserver.org",
            "verify_keys": {
                "ed25519:auto": {
                    "key": "abcd1234567890"
                }
            },
            "old_verify_keys": {},
            "valid_until_ts": 1234567890123i64
        });

        let canonical = to_canonical_json(&server_object).unwrap();
        
        // Keys should be sorted: old_verify_keys, server_name, valid_until_ts, verify_keys
        let expected = r#"{"old_verify_keys":{},"server_name":"example.homeserver.org","valid_until_ts":1234567890123,"verify_keys":{"ed25519:auto":{"key":"abcd1234567890"}}}"#;
        
        assert_eq!(canonical, expected);
        
        // Verify it's different from standard JSON (which doesn't guarantee key order)
        let standard_json = serde_json::to_string(&server_object).unwrap();
        
        // Parse both back to ensure they contain the same data
        let canonical_parsed: Value = serde_json::from_str(&canonical).unwrap();
        let standard_parsed: Value = serde_json::from_str(&standard_json).unwrap();
        
        assert_eq!(canonical_parsed, standard_parsed);
        // Canonical JSON validated - ready for cryptographic signatures
    }

    /// Test canonical JSON with the notary signature structure
    /// This verifies that notary signatures will work correctly 
    #[test]
    fn test_notary_signature_canonical_json() {
        // This is the structure used in by_server_name.rs:create_notary_signature()
        let notary_data = json!({
            "server_name": "remote.matrix.org", 
            "verify_keys": {
                "ed25519:1": {
                    "key": "remote_key_here"
                }
            },
            "old_verify_keys": {},
            "valid_until_ts": 9876543210123i64
            // Note: signatures field is removed before canonicalization in the actual code
        });

        let canonical = to_canonical_json(&notary_data).unwrap();
        
        // Keys should be sorted alphabetically
        let expected = r#"{"old_verify_keys":{},"server_name":"remote.matrix.org","valid_until_ts":9876543210123,"verify_keys":{"ed25519:1":{"key":"remote_key_here"}}}"#;
        
        assert_eq!(canonical, expected);
        // Notary signature canonical JSON validated
    }

    /// Test that canonical JSON produces deterministic output
    #[test]
    fn test_canonical_json_deterministic() {
        let test_object = json!({
            "zebra": "animal",
            "apple": "fruit",
            "beta": "greek_letter", 
            "nested": {
                "z": 26,
                "a": 1
            }
        });

        // Generate canonical JSON multiple times
        let canonical1 = to_canonical_json(&test_object).unwrap();
        let canonical2 = to_canonical_json(&test_object).unwrap();
        let canonical3 = to_canonical_json(&test_object).unwrap();

        // All should be identical (deterministic)
        assert_eq!(canonical1, canonical2);
        assert_eq!(canonical2, canonical3);

        // Verify key ordering
        assert!(canonical1.starts_with(r#"{"apple":"#));
        assert!(canonical1.contains(r#""beta":"#));
        assert!(canonical1.contains(r#""nested":{"a":1,"z":26}"#));
        assert!(canonical1.ends_with(r#""zebra":"animal"}"#));
        
        // Deterministic output verified for cryptographic consistency
    }
}