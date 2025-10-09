# Rust Error Handling Patterns - Research Document

## Overview
This document provides research and best practices for error handling in Rust, specifically focusing on the differences between `.unwrap()`, `.expect()`, and the `?` operator.

## The Problem with .unwrap()

`.unwrap()` is a method that:
- Panics immediately if the Result is an Err or the Option is None
- Provides NO context about what went wrong
- Crashes the entire process
- Makes debugging extremely difficult

### Example of Poor Error Message
```rust
let url = Url::parse("https://example.com").unwrap();
// If this fails, you get:
// thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: ...'
// No indication of WHICH unwrap failed or WHY
```

## The Solution: .expect()

`.expect()` is identical to `.unwrap()` except it allows you to provide a descriptive error message:

### Example of Good Error Message
```rust
let url = Url::parse("https://example.com")
    .expect("Failed to parse homeserver URL");
// If this fails, you get:
// thread 'main' panicked at 'Failed to parse homeserver URL: ...'
// Clear indication of what failed
```

### When to Use .expect()
1. **Test Code**: Tests should fail fast with clear messages
2. **Compile-time Constants**: When parsing hardcoded values that are programmer errors if invalid
3. **Initialization Code**: When failure should be immediate and obvious

## The Best Solution: ? Operator

The `?` operator propagates errors to the caller, allowing proper error handling:

### Example
```rust
pub fn parse_config(url: &str) -> Result<Config, ConfigError> {
    let parsed_url = Url::parse(url)?;  // Returns error to caller
    Ok(Config { url: parsed_url })
}
```

### When to Use ?
1. **Production Code**: Any code that can reasonably fail
2. **Library Functions**: Let callers decide how to handle errors
3. **Error Recovery**: When the caller might want to retry or provide fallback

## Pattern Matrix

| Situation | Use | Rationale |
|-----------|-----|-----------|
| Test code | `.expect("descriptive message")` | Fast failure with context |
| Production error handling | `?` operator | Proper error propagation |
| Compile-time constants | `.expect("why this should never fail")` | Document assumptions |
| Library APIs | `Result<T, E>` with `?` | Let callers handle errors |
| Hardcoded URLs/values | `.expect()` | Programmer error if invalid |

## Real-World Examples from Matryx

### Test Code Pattern (canonical_json.rs)
```rust
#[test]
fn test_canonical_json_object_key_sorting() {
    let data = json!({
        "z_last": "value_z",
        "a_first": "value_a"
    });
    
    let result = canonical_json(&data)
        .expect("Failed to canonicalize test JSON");
    // Clear message: tells you WHAT failed and WHERE
}
```

### Production Constant Pattern (client lib.rs)
```rust
impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            homeserver_url: Url::parse("https://matrix.example.com")
                .expect("Default homeserver URL should be valid"),
            // This is a programmer error if it fails - the URL is hardcoded
        }
    }
}
```

### Production Error Handling Pattern (canonical_json.rs)
```rust
pub fn canonical_json(value: &Value) -> Result<String, CanonicalJsonError> {
    let canonical_value = canonicalize_value(value)?;  // Propagate errors
    serde_json::to_string(&canonical_value)
        .map_err(|e| CanonicalJsonError::SerializationError(e.to_string()))
}
```

## Clippy Lint: unwrap_used

The `#![deny(clippy::unwrap_used)]` lint prevents accidental use of `.unwrap()`:

```rust
#![deny(clippy::unwrap_used)]  // Add to top of lib.rs

// This will now cause a compilation error:
let x = some_result.unwrap();  // ❌ Compilation error

// But these are allowed:
let x = some_result.expect("reason");  // ✅ Allowed
let x = some_result?;  // ✅ Allowed
```

## Key Takeaways

1. **Never use .unwrap() in production code** - it provides no context
2. **Always use .expect() in test code** - fast failure with clear messages
3. **Use ? operator for error propagation** - proper error handling
4. **Add clippy::unwrap_used lint** - prevent future mistakes
5. **Document why .expect() is safe** - explain your assumptions

## References

- [Rust Book - Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [Clippy unwrap_used Lint](https://rust-lang.github.io/rust-clippy/master/index.html#unwrap_used)
- [Rust API Guidelines - Error Handling](https://rust-lang.github.io/api-guidelines/dependability.html#error-handling)
