# ERRFIX_1: Improve Error Handling and Remove Generic Fallbacks

**Status**: Ready for Implementation  
**Priority**: MEDIUM  
**Estimated Effort**: 2-3 days  
**Package**: packages/client, packages/server, packages/entity

---

## OBJECTIVE

Replace generic fallback error handling with proper error types and logging to improve debuggability and prevent loss of error information.

---

## CODEBASE CONTEXT

### Current State Analysis

After examining the codebase, here's what currently exists:

**1. HTTP Client Error Handling** ([`packages/client/src/http_client.rs`](../../packages/client/src/http_client.rs))
- HttpClientError enum is defined at **lines 18-40** in http_client.rs itself (NOT in a separate error.rs file)
- The problematic fallback is in `parse_matrix_error()` method at **lines 149-158**
- Current variants: Network, Matrix, Serialization, InvalidUrl, AuthenticationRequired, MaxRetriesExceeded
- Missing: InvalidResponse variant for parse errors

**2. Mentions Fallback** ([`packages/server/src/mentions.rs`](../../packages/server/src/mentions.rs))
- The fallback logic is at **lines 145-156** 
- Currently has minimal comment: "Detect mentions from content text (fallback for backwards compatibility)"
- This code is CORRECT and working - just needs better documentation
- Related to Matrix Spec MSC3952 (m.mentions field)

**3. Event Content Unknown Variant** ([`packages/entity/src/types/event_content.rs`](../../packages/entity/src/types/event_content.rs))
- Unknown variant at **line 49**
- Current comment: "Generic fallback for unknown event types"
- This is intentional Matrix extensibility - needs clarification in documentation

**4. Logging Infrastructure**
- Tracing crate v0.1.41 already in dependencies
- Codebase uses simple macro forms: `info!("message")`, `warn!("message with {}", value)`
- NO target specification used in most places (keep it simple)

---

## PROBLEM DESCRIPTION

Several locations use generic fallback error handling that masks real errors:

1. **HTTP Client** (`packages/client/src/http_client.rs:149-158`): Non-Matrix errors converted to generic Matrix errors
2. **Mentions Fallback** (`packages/server/src/mentions.rs:145-156`): Actually GOOD - needs documentation only
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

**Matrix Specification References**:
- MSC3952: Intentional mentions (m.mentions field) - newer clients include this
- Backwards compatibility: Older clients don't send m.mentions, servers must parse text
- Event types: Matrix allows custom event types (com.example.custom_event)

---

## SUBTASK 1: Improve HTTP Client Error Handling

**Objective**: Replace generic M_UNKNOWN fallback with proper error types.

**Location**: [`packages/client/src/http_client.rs`](../../packages/client/src/http_client.rs)

### CURRENT CODE STATE

**Lines 18-40: HttpClientError enum definition**
```rust
/// HTTP client errors with Matrix-spec error handling
#[derive(Debug, thiserror::Error)]
pub enum HttpClientError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Matrix error {errcode}: {error} (HTTP {status})")]
    Matrix {
        status: u16,
        errcode: String,
        error: String,
        retry_after_ms: Option<u64>,
    },

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("Authentication required")]
    AuthenticationRequired,

    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
}
```

**Lines 149-158: Problematic parse_matrix_error method**
```rust
fn parse_matrix_error<T>(&self, status: u16, body: &str) -> Result<T, HttpClientError> {
    match serde_json::from_str::<MatrixErrorResponse>(body) {
        Ok(matrix_err) => Err(HttpClientError::Matrix {
            status,
            errcode: matrix_err.errcode,
            error: matrix_err.error,
            retry_after_ms: matrix_err.retry_after_ms,
        }),
        Err(_) => {
            // Fallback: non-JSON error response
            Err(HttpClientError::Matrix {
                status,
                errcode: "M_UNKNOWN".to_string(),
                error: body.to_string(),
                retry_after_ms: None,
            })
        }
    }
}
```

### PROBLEMS

- Parse error information is discarded (`Err(_)` throws away details)
- Body content might not be suitable error message (could be HTML, plain text, etc.)
- No indication this is a parse failure vs real M_UNKNOWN from server
- Creates fake Matrix error when response isn't Matrix format

### REQUIRED CHANGES

**Step 1: Add InvalidResponse variant to HttpClientError enum (lines 18-40)**

Insert this NEW variant after the Matrix variant:

```rust
#[derive(Debug, thiserror::Error)]
pub enum HttpClientError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Matrix error {errcode}: {error} (HTTP {status})")]
    Matrix {
        status: u16,
        errcode: String,
        error: String,
        retry_after_ms: Option<u64>,
    },

    /// Response parsing failed (not valid Matrix error format)
    /// 
    /// This indicates the server returned an error response that doesn't
    /// follow Matrix error format. The body and parse error are preserved
    /// for debugging.
    #[error("Invalid response format (status {status}): {parse_error}")]
    InvalidResponse {
        status: u16,
        body: String,
        parse_error: String,
    },

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    // ... rest of variants
}
```

**Step 2: Update parse_matrix_error method (lines 149-158)**

Replace the current method with this improved version:

```rust
fn parse_matrix_error<T>(&self, status: u16, body: &str) -> Result<T, HttpClientError> {
    match serde_json::from_str::<MatrixErrorResponse>(body) {
        Ok(matrix_err) => {
            Err(HttpClientError::Matrix {
                status,
                errcode: matrix_err.errcode,
                error: matrix_err.error,
                retry_after_ms: matrix_err.retry_after_ms,
            })
        }
        Err(parse_err) => {
            // Log parse error with full details for debugging
            tracing::warn!(
                "Failed to parse error response as Matrix error (status {}): {}",
                status,
                parse_err
            );
            tracing::debug!("Response body: {}", body);

            // Return InvalidResponse error with preserved information
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
}
```

### FILES TO MODIFY

- [`packages/client/src/http_client.rs`](../../packages/client/src/http_client.rs)
  - Lines 18-40: Add InvalidResponse variant to enum
  - Lines 149-158: Update parse_matrix_error method
  - Note: HttpClientError is defined HERE, not in a separate error.rs file

### DEFINITION OF DONE

- InvalidResponse error variant added to HttpClientError enum
- Parse errors logged with `warn!` level and details
- Response body included in error (truncated if > 200 chars)
- Response body logged at `debug!` level (full content)
- No information loss from original parse error
- Code compiles without errors

---

## SUBTASK 2: Document Intentional Fallbacks

**Objective**: Add documentation to legitimate fallback cases explaining they're intentional.

### Location 1: Mentions Fallback

**File**: [`packages/server/src/mentions.rs`](../../packages/server/src/mentions.rs)  
**Lines**: 145-156

#### CURRENT CODE STATE

```rust
} else {
    // Detect mentions from content text (fallback for backwards compatibility)
    mentioned_users.extend(self.detect_user_mentions(&text_content, room_id, state).await?);
    has_room_mention = self.detect_room_mentions(&text_content);

    // Detect room alias mentions for cross-room context
    room_alias_mentions = self.detect_and_resolve_room_alias_mentions(&text_content, state).await;
    if !room_alias_mentions.is_empty() {
        info!("Detected and resolved room alias mentions in room {}: {:?}", room_id, room_alias_mentions);
        // Note: Room aliases are stored in custom metadata field, not m.mentions
        // This is intentional - m.mentions is spec-defined for user/@room only
    }
}
```

#### REQUIRED CHANGE

Replace the minimal comment with comprehensive documentation:

```rust
} else {
    // Fallback to text-based mention detection for backwards compatibility.
    //
    // This is INTENTIONAL per Matrix specification behavior (MSC3952):
    // - Clients that don't support MSC3952 (m.mentions field) will not include it
    // - Servers MUST parse message body text to detect @mentions and @room pings
    // - This ensures mentions work with older/minimal Matrix clients
    // - Modern clients should include m.mentions, but we support both approaches
    //
    // This is NOT a workaround or incomplete code - it's required backwards
    // compatibility as specified by Matrix protocol.
    //
    // References:
    // - MSC3952: Intentional Mentions
    // - Matrix Client-Server API Spec (room events)
    mentioned_users.extend(self.detect_user_mentions(&text_content, room_id, state).await?);
    has_room_mention = self.detect_room_mentions(&text_content);

    // Detect room alias mentions for cross-room context
    room_alias_mentions = self.detect_and_resolve_room_alias_mentions(&text_content, state).await;
    if !room_alias_mentions.is_empty() {
        info!("Detected and resolved room alias mentions in room {}: {:?}", room_id, room_alias_mentions);
        // Note: Room aliases are stored in custom metadata field, not m.mentions
        // This is intentional - m.mentions is spec-defined for user/@room only
    }
}
```

### Location 2: Event Content Unknown Variant

**File**: [`packages/entity/src/types/event_content.rs`](../../packages/entity/src/types/event_content.rs)  
**Lines**: 49

#### CURRENT CODE STATE

```rust
/// Server notice content (m.server_notice)
ServerNotice(ServerNoticeContent),

/// Generic fallback for unknown event types
Unknown(serde_json::Value),
```

#### REQUIRED CHANGE

Replace with comprehensive documentation:

```rust
/// Server notice content (m.server_notice)
ServerNotice(ServerNoticeContent),

/// Unknown/custom event content types
///
/// Matrix allows custom event types and future event types not yet
/// implemented in this codebase. This catch-all variant preserves
/// the event content for:
/// 
/// - **Custom event types**: Application-specific events like "com.example.custom_event"
/// - **Future Matrix events**: New event types added to spec after this code was written
/// - **Third-party integrations**: Events from bridges, bots, or other services
/// - **Experimental features**: Events from MSCs (Matrix Spec Changes) not yet finalized
///
/// The raw JSON value is preserved so custom handling can access it.
/// This is INTENTIONAL extensibility per Matrix specification - NOT
/// incomplete implementation.
///
/// Matrix Specification: "Clients and servers MUST be able to handle
/// unknown event types gracefully by preserving their content."
Unknown(serde_json::Value),
```

### FILES TO MODIFY

- [`packages/server/src/mentions.rs`](../../packages/server/src/mentions.rs) - Line 145
- [`packages/entity/src/types/event_content.rs`](../../packages/entity/src/types/event_content.rs) - Line 49

### DEFINITION OF DONE

- Mentions fallback has comprehensive comment explaining intentionality
- References Matrix spec (MSC3952) and backwards compatibility requirement
- Event content Unknown variant has detailed documentation
- Future maintainers won't mistake these for incomplete code
- Comments clearly distinguish intentional design from bugs/workarounds

---

## SUBTASK 3: Add Structured Logging for Error Paths

**Objective**: Ensure all error paths in http_client.rs have adequate logging for debugging.

**Location**: [`packages/client/src/http_client.rs`](../../packages/client/src/http_client.rs)

### IMPLEMENTATION PATTERN

The codebase uses simple tracing macros without target specification. Follow these patterns:

**For parse errors** (already shown in SUBTASK 1):
```rust
Err(parse_err) => {
    tracing::warn!(
        "Failed to parse error response as Matrix error (status {}): {}",
        status,
        parse_err
    );
    tracing::debug!("Response body: {}", body);
    // ... create InvalidResponse error
}
```

**For network errors** (add to request() method if not present):
```rust
Err(e) => {
    tracing::error!(
        "HTTP request failed: {} {} - {}",
        method,
        url,
        e
    );
    return Err(HttpClientError::Network(e));
}
```

**For timeout errors** (if timeout handling is added):
```rust
if elapsed > timeout {
    tracing::warn!(
        "Request timeout after {}ms: {} {}",
        elapsed.as_millis(),
        method,
        url
    );
    return Err(HttpClientError::Timeout);
}
```

### LOGGING GUIDELINES

**Log Levels**:
- `error!`: Request failures, critical errors
- `warn!`: Parse failures, retryable errors, timeouts
- `debug!`: Response bodies, detailed context (can be verbose)

**Security**:
- NEVER log: access_token, passwords, sensitive headers
- Truncate large bodies (> 200 chars) at warn/error level
- Full bodies only at debug level

**Context**:
- Include: HTTP method, URL, status code, error type
- For retries: Include attempt number, delay
- For rate limits: Include retry_after_ms if available

### FILES TO MODIFY

- [`packages/client/src/http_client.rs`](../../packages/client/src/http_client.rs) - All error paths

### DEFINITION OF DONE

- All error paths in http_client.rs have structured logging
- Log levels appropriate (error for failures, warn for recoverable, debug for details)
- Sensitive data (tokens, passwords) not logged
- Response bodies truncated at warn/error, full at debug
- Enough context to debug production issues

---

## SUBTASK 4: Add Error Context Helpers

**Objective**: Create helper methods to check error properties and add context.

**Location**: [`packages/client/src/http_client.rs`](../../packages/client/src/http_client.rs)

### IMPLEMENTATION

Add these methods to the HttpClientError impl block (create one if it doesn't exist):

```rust
impl HttpClientError {
    /// Check if error is retryable (network issues, 5xx, rate limits)
    pub fn is_retryable(&self) -> bool {
        match self {
            HttpClientError::Matrix { status, errcode, .. } => {
                // Rate limits are retryable, auth errors are not
                *status >= 500 || errcode == "M_LIMIT_EXCEEDED"
            }
            HttpClientError::Network(_) => true,
            HttpClientError::InvalidResponse { status, .. } => {
                // 5xx server errors might be transient
                *status >= 500
            }
            HttpClientError::AuthenticationRequired => false,
            HttpClientError::MaxRetriesExceeded => false,
            HttpClientError::Serialization(_) => false,
            HttpClientError::InvalidUrl(_) => false,
        }
    }

    /// Get retry delay if error is retryable
    pub fn retry_delay(&self) -> Option<std::time::Duration> {
        use std::time::Duration;
        
        match self {
            HttpClientError::Matrix { retry_after_ms: Some(ms), .. } => {
                Some(Duration::from_millis(*ms))
            }
            HttpClientError::Network(_) => Some(Duration::from_secs(1)),
            HttpClientError::InvalidResponse { status, .. } if *status >= 500 => {
                Some(Duration::from_secs(2))
            }
            _ => None,
        }
    }

    /// Get HTTP status code if available
    pub fn status_code(&self) -> Option<u16> {
        match self {
            HttpClientError::Matrix { status, .. } => Some(*status),
            HttpClientError::InvalidResponse { status, .. } => Some(*status),
            _ => None,
        }
    }

    /// Check if error is a client error (4xx)
    pub fn is_client_error(&self) -> bool {
        match self.status_code() {
            Some(status) => (400..500).contains(&status),
            None => false,
        }
    }

    /// Check if error is a server error (5xx)
    pub fn is_server_error(&self) -> bool {
        match self.status_code() {
            Some(status) => status >= 500,
            None => false,
        }
    }
}
```

### FILES TO MODIFY

- [`packages/client/src/http_client.rs`](../../packages/client/src/http_client.rs)

### DEFINITION OF DONE

- `is_retryable()` method correctly identifies retryable errors
- `retry_delay()` returns appropriate delay for retryable errors
- `status_code()` extracts HTTP status when available
- `is_client_error()` and `is_server_error()` categorize errors
- All methods have clear documentation
- Code compiles without errors

---

## CONSTRAINTS

- Do NOT write unit tests, integration tests, or test fixtures
- Do NOT write benchmark code
- Do NOT create documentation files (README, etc.)
- FOCUS ON: Production code changes only

---

## DEPENDENCIES

**Rust Crates** (already in Cargo.toml):
- `tracing = "0.1.41"` - Structured logging (already present)
- `serde_json = "1.0.145"` - JSON parsing (already present)
- `thiserror = "2.0.17"` - Error derive macros (already present)

**No new dependencies needed** - everything is already available.

---

## DEFINITION OF DONE

- [ ] InvalidResponse error variant added to HttpClientError (http_client.rs lines 18-40)
- [ ] HTTP client parse_matrix_error improved with proper logging (lines 149-158)
- [ ] Parse errors logged with full details (warn + debug levels)
- [ ] Intentional fallback in mentions.rs documented (line 145)
- [ ] Event content Unknown variant documented (event_content.rs line 49)
- [ ] All error paths in http_client.rs have structured logging
- [ ] Helper methods added: is_retryable(), retry_delay(), status_code(), etc.
- [ ] No information loss from original errors
- [ ] No compilation errors
- [ ] Logging follows codebase style (simple macros, no targets)

---

## FILES TO MODIFY

1. [`packages/client/src/http_client.rs`](../../packages/client/src/http_client.rs)
   - Lines 18-40: Add InvalidResponse variant to HttpClientError enum
   - Lines 149-158: Update parse_matrix_error with logging
   - Add impl HttpClientError block with helper methods
   - Add logging to error paths throughout

2. [`packages/server/src/mentions.rs`](../../packages/server/src/mentions.rs)
   - Line 145: Enhance documentation (comment only, no code changes)

3. [`packages/entity/src/types/event_content.rs`](../../packages/entity/src/types/event_content.rs)
   - Line 49: Enhance documentation (comment only, no code changes)

---

## NOTES

**Error Handling Best Practices**:
- Preserve all error information - never discard parse errors
- Add context without losing original error
- Use appropriate log levels (error/warn/debug)
- Don't log sensitive data (tokens, passwords, etc.)
- Truncate large bodies in logs to prevent overwhelming output

**Matrix Specification References**:
- MSC3952: Intentional Mentions - explains m.mentions field behavior
- Matrix Client-Server API: Defines error response format
- Extensibility: Matrix allows custom event types and unknown fields

**Implementation Strategy**:
1. Start with SUBTASK 1 (HTTP client errors) - most impactful
2. Then SUBTASK 4 (helper methods) - builds on SUBTASK 1
3. Then SUBTASK 3 (logging) - uses patterns from SUBTASK 1
4. Finally SUBTASK 2 (documentation) - simple comment updates

**Why This Matters**:
- Good error messages save hours of debugging time
- Structured logging enables quick production issue diagnosis
- Clear documentation prevents future "fixes" to intentional behavior
- Proper error types enable better error handling in calling code
