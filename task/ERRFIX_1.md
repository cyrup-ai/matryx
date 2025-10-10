# ERRFIX_1: Improve Error Handling and Remove Generic Fallbacks

**Status**: Ready for Implementation
**Priority**: MEDIUM
**Estimated Effort**: 2-3 days
**Package**: packages/client, packages/server

---

## OBJECTIVE

Replace generic fallback error handling with proper error types and logging to improve debuggability and prevent loss of error information.

---

## PROBLEM DESCRIPTION

Several locations use generic fallback error handling that masks real errors:

1. **HTTP Client** (`packages/client/src/http_client.rs:150`): Non-Matrix errors converted to generic Matrix errors
2. **Mentions Fallback** (`packages/server/src/mentions.rs:145`): Actually GOOD - needs documentation only
3. **Event Content Fallback** (`packages/entity/src/types/event_content.rs:49`): Legitimate catch-all for unknown types

**Impact of Poor Error Handling**:
- Real error information lost during debugging
- Non-Matrix errors misclassified as Matrix errors
- Difficult to diagnose production issues
- Error logs lack sufficient detail

---

## RESEARCH NOTES

**Matrix Error Format**:
- Standard errors have `errcode` and `error` fields
- Common codes: M_FORBIDDEN, M_NOT_FOUND, M_LIMIT_EXCEEDED, M_UNKNOWN, etc.
- Non-Matrix endpoints may return different error formats

**Good Fallback vs Bad Fallback**:
- **Good**: Backwards compatibility (mentions.rs) - intentional per spec
- **Bad**: Hiding errors (http_client.rs) - loses debugging information
- **Acceptable**: Unknown variants (event_content.rs) - catch-all for extensibility

---

## SUBTASK 1: Improve HTTP Client Error Handling

**Objective**: Replace generic M_UNKNOWN fallback with proper error types.

**Location**: `packages/client/src/http_client.rs` (around line 145-156)

**Current Code**:
```rust
Err(_) => {
    // Fallback: non-Matrix error format
    Err(HttpClientError::Matrix {
        status,
        errcode: "M_UNKNOWN".to_string(),
        error: body.to_string(),
        retry_after_ms: None,
    })
}
```

**Problems**:
- Parse error information is discarded
- Body content might not be suitable error message
- No indication this is a parse failure vs real M_UNKNOWN

**Required Changes**:

1. Add new error variant to HttpClientError enum:
```rust
pub enum HttpClientError {
    // ... existing variants ...

    /// Matrix error with proper error code
    Matrix {
        status: u16,
        errcode: String,
        error: String,
        retry_after_ms: Option<u64>,
    },

    /// Response parsing failed (not valid Matrix error format)
    InvalidResponse {
        status: u16,
        body: String,
        parse_error: String,
    },

    // ... other variants
}
```

2. Update error handling logic:
```rust
match serde_json::from_str::<MatrixError>(&body) {
    Ok(matrix_err) => {
        Err(HttpClientError::Matrix {
            status,
            errcode: matrix_err.errcode,
            error: matrix_err.error,
            retry_after_ms: matrix_err.retry_after_ms,
        })
    }
    Err(parse_err) => {
        // Log the parse error for debugging
        tracing::warn!(
            "Failed to parse error response as Matrix error (status {}): {}",
            status,
            parse_err
        );
        tracing::debug!("Response body: {}", body);

        // Return InvalidResponse error
        Err(HttpClientError::InvalidResponse {
            status,
            body: if body.len() > 200 {
                format!("{}... (truncated)", &body[..200])
            } else {
                body.to_string()
            },
            parse_error: parse_err.to_string(),
        })
    }
}
```

3. Update Display impl for HttpClientError:
```rust
impl std::fmt::Display for HttpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpClientError::Matrix { status, errcode, error, .. } => {
                write!(f, "Matrix error ({}): {} - {}", status, errcode, error)
            }
            HttpClientError::InvalidResponse { status, body, parse_error } => {
                write!(
                    f,
                    "Invalid response format (status {}): parse error: {}, body: {}",
                    status, parse_error, body
                )
            }
            // ... other variants
        }
    }
}
```

**Files to Modify**:
- `packages/client/src/http_client.rs` (lines 145-156)
- `packages/client/src/error.rs` (or wherever HttpClientError is defined)

**Definition of Done**:
- InvalidResponse error variant added
- Parse errors logged with details
- Response body included in error (truncated if long)
- No information loss from original error
- Display impl updated for new variant

---

## SUBTASK 2: Document Intentional Fallbacks

**Objective**: Add documentation to legitimate fallback cases explaining they're intentional.

**Location 1**: `packages/server/src/mentions.rs:145`

**Current Code**:
```rust
} else {
    // Fallback to text-based detection if m.mentions not present
    mentioned_users.extend(self.detect_user_mentions(&text_content, room_id, state).await?);
    has_room_mention = self.detect_room_mentions(&text_content);

    // Detect room alias mentions for cross-room context
    mentioned_room_aliases.extend(self.detect_room_alias_mentions(&text_content).await?);
}
```

**Required Documentation**:
```rust
} else {
    // Fallback to text-based mention detection for backwards compatibility.
    //
    // This is INTENTIONAL per Matrix specification behavior:
    // - Clients that don't support MSC3952 (m.mentions field) will not include it
    // - Servers must parse message body text to detect @mentions and room pings
    // - This ensures mentions work with older/minimal Matrix clients
    //
    // This is NOT a workaround - it's required backwards compatibility.
    mentioned_users.extend(self.detect_user_mentions(&text_content, room_id, state).await?);
    has_room_mention = self.detect_room_mentions(&text_content);
    mentioned_room_aliases.extend(self.detect_room_alias_mentions(&text_content).await?);
}
```

**Files to Modify**:
- `packages/server/src/mentions.rs` (line 145)

**Definition of Done**:
- Comment clearly explains this is intentional
- References Matrix specification behavior
- Clarifies this is backwards compatibility, not a bug
- Future maintainers won't mistake this for incomplete code

---

**Location 2**: `packages/entity/src/types/event_content.rs:49`

**Current Code**:
```rust
/// Server notice content (m.server_notice)
ServerNotice(ServerNoticeContent),

// Fallback for unknown event types
Unknown(serde_json::Value),
```

**Required Documentation**:
```rust
/// Server notice content (m.server_notice)
ServerNotice(ServerNoticeContent),

/// Unknown/custom event content types
///
/// Matrix allows custom event types and future event types not yet
/// implemented in this codebase. This catch-all variant preserves
/// the event content for:
/// - Custom event types (e.g., "com.example.custom_event")
/// - Future Matrix specification event types
/// - Events from newer servers/clients
///
/// The raw JSON value is preserved so custom handling can access it.
/// This is INTENTIONAL extensibility per Matrix specification.
Unknown(serde_json::Value),
```

**Files to Modify**:
- `packages/entity/src/types/event_content.rs` (line 49)

**Definition of Done**:
- Comment explains Unknown variant purpose
- Clarifies this enables Matrix extensibility
- Documents when this variant is used
- Not marked as incomplete or TODO

---

## SUBTASK 3: Add Structured Logging for Error Paths

**Objective**: Ensure all error paths have adequate logging for debugging.

**Location**: Anywhere InvalidResponse errors are created

**Implementation Pattern**:

For network errors:
```rust
Err(e) => {
    tracing::error!(
        target: "matryx_client::http",
        error = %e,
        method = %request.method(),
        url = %request.url(),
        "HTTP request failed"
    );
    return Err(HttpClientError::Network(e));
}
```

For parse errors:
```rust
Err(parse_err) => {
    tracing::warn!(
        target: "matryx_client::http",
        status = status,
        parse_error = %parse_err,
        body_len = body.len(),
        "Failed to parse response as Matrix error"
    );
    tracing::debug!(
        target: "matryx_client::http",
        body = %body,
        "Full response body"
    );
    // ... create InvalidResponse error
}
```

For timeout errors:
```rust
if elapsed > timeout {
    tracing::warn!(
        target: "matryx_client::http",
        elapsed_ms = elapsed.as_millis(),
        timeout_ms = timeout.as_millis(),
        url = %request.url(),
        "Request timeout"
    );
    return Err(HttpClientError::Timeout);
}
```

**Files to Modify**:
- `packages/client/src/http_client.rs` (all error paths)

**Definition of Done**:
- All error paths have structured logging
- Log levels appropriate (error for failures, warn for recoverable, debug for details)
- Sensitive data (tokens, passwords) not logged
- Enough context to debug issues in production
- Body content logged at debug level only (can be large)

---

## SUBTASK 4: Add Error Context Helpers

**Objective**: Create helper functions to add context to errors.

**Location**: `packages/client/src/http_client.rs` or `packages/client/src/error.rs`

**Implementation**:

Add context methods to HttpClientError:
```rust
impl HttpClientError {
    /// Add context about the request that failed
    pub fn with_request_context(self, method: &str, url: &str) -> Self {
        match self {
            HttpClientError::InvalidResponse { status, body, parse_error } => {
                HttpClientError::InvalidResponse {
                    status,
                    body: format!(
                        "{} (request: {} {})",
                        body, method, url
                    ),
                    parse_error,
                }
            }
            // Pass through other variants unchanged
            other => other,
        }
    }

    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            HttpClientError::Matrix { errcode, .. } => {
                // Rate limits are retryable, auth errors are not
                errcode == "M_LIMIT_EXCEEDED"
            }
            HttpClientError::Network(_) => true,
            HttpClientError::Timeout => true,
            HttpClientError::InvalidResponse { .. } => false,
            _ => false,
        }
    }

    /// Get retry delay if applicable
    pub fn retry_delay(&self) -> Option<Duration> {
        match self {
            HttpClientError::Matrix { retry_after_ms: Some(ms), .. } => {
                Some(Duration::from_millis(*ms))
            }
            HttpClientError::Network(_) => Some(Duration::from_secs(1)),
            HttpClientError::Timeout => Some(Duration::from_secs(5)),
            _ => None,
        }
    }
}
```

**Files to Modify**:
- `packages/client/src/error.rs` (or wherever HttpClientError is defined)

**Definition of Done**:
- Helper methods for adding request context
- Methods to check if error is retryable
- Methods to get retry delay
- Documentation on each method

---

## CONSTRAINTS

⚠️ **NO TESTS**: Do not write unit tests, integration tests, or test fixtures. Test team handles all testing.

⚠️ **NO BENCHMARKS**: Do not write benchmark code. Performance team handles benchmarking.

⚠️ **FOCUS ON FUNCTIONALITY**: Only modify production code in ./src directories.

---

## DEPENDENCIES

**Rust Crates** (likely already in dependencies):
- tracing (for structured logging)
- serde_json (for error parsing)

**Error Handling Best Practices**:
- Preserve all error information
- Add context without losing original error
- Use appropriate log levels
- Don't log sensitive data

---

## DEFINITION OF DONE

- [ ] InvalidResponse error variant added to HttpClientError
- [ ] HTTP client error fallback improved with proper logging
- [ ] Parse errors logged with full details
- [ ] Intentional fallbacks (mentions, event content) documented
- [ ] All error paths have structured logging
- [ ] Helper methods added for error context and retryability
- [ ] No information loss from original errors
- [ ] No compilation errors
- [ ] No test code written
- [ ] No benchmark code written

---

## FILES TO MODIFY

1. `packages/client/src/http_client.rs` (lines 145-156 + error handling throughout)
2. `packages/client/src/error.rs` (add InvalidResponse variant + helpers)
3. `packages/server/src/mentions.rs` (line 145 - documentation only)
4. `packages/entity/src/types/event_content.rs` (line 49 - documentation only)

---

## NOTES

- Error handling is critical for debugging production issues
- Logging should be structured (key-value pairs) for easy parsing
- Balance between too much logging (noise) and too little (no debug info)
- Use tracing crate's structured logging features
- Response bodies can be large - truncate or log at debug level only
- This task improves maintainability and debuggability
- Good error messages save hours of debugging time
