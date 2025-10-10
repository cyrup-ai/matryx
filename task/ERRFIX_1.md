# ERRFIX_1: Fix Retry Logic Inconsistency

**Status**: NEARLY COMPLETE - 1 Critical Issue Remaining  
**Priority**: HIGH  
**Estimated Effort**: 15 minutes  
**Package**: packages/client

---

## IMPLEMENTATION RATING: 7/10

**COMPLETED WORK (Excellent):**
- ✅ InvalidResponse error variant added with comprehensive documentation
- ✅ parse_matrix_error improved with proper warn/debug logging
- ✅ All error information preserved (no loss)
- ✅ Mentions.rs intentional fallback documented comprehensively
- ✅ Event content Unknown variant documented comprehensively
- ✅ Helper methods implemented: is_retryable(), retry_delay(), status_code(), is_client_error(), is_server_error()
- ✅ Logging follows codebase style (simple macros, no targets)
- ✅ All error paths have structured logging

**CRITICAL ISSUE REMAINING:**

The retry logic in `request_with_retry()` doesn't use the `is_retryable()` helper method, creating **dangerous inconsistency**:

---

## THE PROBLEM

**File**: `/Volumes/samsung_t9/maxtryx/packages/client/src/http_client.rs`  
**Lines**: 310-318

### Current Code (INCONSISTENT)
```rust
// Check if we should retry
let should_retry = match &e {
    // Retry on network errors
    HttpClientError::Network(_) => true,
    // Retry on 5xx server errors or rate limit
    HttpClientError::Matrix { status, errcode, .. } => {
        *status >= 500 || errcode == "M_LIMIT_EXCEEDED"
    }
    // Don't retry on 4xx client errors (except rate limit)
    _ => false,
};
```

### Why This Is Wrong

1. **InvalidResponse with 5xx won't be retried**
   - The helper method `is_retryable()` says InvalidResponse with 5xx status SHOULD be retried
   - But the catch-all `_ => false` means they WON'T be retried
   - This defeats the purpose of adding InvalidResponse variant

2. **Duplicate logic creates maintenance risk**
   - Retry logic is duplicated in two places
   - If `is_retryable()` is updated, retry logic won't reflect the change
   - Violates DRY principle

3. **Helper methods not actually used**
   - The task added helper methods but didn't integrate them
   - This is incomplete implementation

---

## THE SOLUTION

Replace the manual match statement with the helper method:

### Required Change

**Location**: Lines 310-318 in `/Volumes/samsung_t9/maxtryx/packages/client/src/http_client.rs`

**Replace this:**
```rust
// Check if we should retry
let should_retry = match &e {
    // Retry on network errors
    HttpClientError::Network(_) => true,
    // Retry on 5xx server errors or rate limit
    HttpClientError::Matrix { status, errcode, .. } => {
        *status >= 500 || errcode == "M_LIMIT_EXCEEDED"
    }
    // Don't retry on 4xx client errors (except rate limit)
    _ => false,
};
```

**With this:**
```rust
// Use helper method for consistent retry logic
let should_retry = e.is_retryable();
```

---

## WHY THIS MATTERS

**Before the fix:**
- Server returns 503 Service Unavailable with HTML error page (not JSON)
- parse_matrix_error creates InvalidResponse with status=503
- Retry logic sees InvalidResponse, falls through to `_ => false`
- **Request fails immediately instead of retrying**
- User sees error that could have been resolved with retry

**After the fix:**
- Same scenario
- Retry logic calls `e.is_retryable()`
- Helper method checks: `status >= 500` → true
- **Request is retried with exponential backoff**
- User experience improves

---

## DEFINITION OF DONE

- [ ] Replace manual match with `e.is_retryable()` call at line 310
- [ ] Verify InvalidResponse errors with 5xx status are now retried
- [ ] Code compiles without errors
- [ ] Single source of truth for retry logic

---

## FILES TO MODIFY

1. `/Volumes/samsung_t9/maxtryx/packages/client/src/http_client.rs`
   - Line 310-318: Replace match statement with helper method call

---

## VERIFICATION

After the fix, this scenario should work correctly:

```rust
// Simulate 503 with HTML error
let error = HttpClientError::InvalidResponse {
    status: 503,
    body: "<html>Service Unavailable</html>".to_string(),
    parse_error: "expected value at line 1 column 1".to_string(),
};

assert!(error.is_retryable()); // Should be true
// And request_with_retry should actually retry it
```

---

## IMPACT

**Severity**: HIGH  
**Risk**: Medium - Current code doesn't retry valid retryable errors  
**Effort**: 5 minutes to fix, 10 minutes to verify  

This is the ONLY remaining issue blocking completion of ERRFIX_1.
