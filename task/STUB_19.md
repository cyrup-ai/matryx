# STUB_19: Unwrap Elimination - Server Core Modules

## OBJECTIVE
Replace all remaining .unwrap() calls in server core modules (auth, config, _matrix endpoints) with proper error handling.

## SCOPE
- `packages/server/src/auth/**/*.rs`
- `packages/server/src/config/**/*.rs`
- `packages/server/src/_matrix/**/*.rs` (selected files with unwraps)
- Remaining unwraps in server package

## SUBTASKS

### SUBTASK 1: Update Config Module Unwraps
- File: `packages/server/src/config/server_config.rs:274`
- Line 274: `let db_url = env::var("DATABASE_URL").unwrap_or_default();`
- This is correct - unwrap_or_default() doesn't panic
- Verify no other unwrap() in config module
- Ensure environment variable parsing uses Result<>

### SUBTASK 2: Review Auth Module
- Files in `packages/server/src/auth/`:
  - Check for unwrap() in JWT handling
  - Check for unwrap() in session validation
  - Check for unwrap() in middleware
- Replace with ? operator and error context
- Authentication errors must be handled gracefully

### SUBTASK 3: Update Client API Endpoints
- Review endpoints with potential unwraps:
  - Login/logout handlers
  - Registration handlers
  - Room creation/joining
  - Message sending
- Pattern for JSON parsing:
  ```rust
  let request: LoginRequest = serde_json::from_value(body)
      .context("Invalid login request body")?;
  ```

### SUBTASK 4: Handle State Event Unwraps
- File paths like `_matrix/client/v3/rooms/by_room_id/state/`
- Event creation may have unwrap() for field access
- Replace with proper Option handling:
  ```rust
  let state_key = event.state_key
      .ok_or_else(|| anyhow!("State event missing state_key"))?;
  ```

### SUBTASK 5: Update Sync Handlers
- File: `packages/server/src/_matrix/client/v3/sync/handlers.rs`
- Check for unwrap() in sync response building
- Timeline event processing
- State resolution calls
- Replace with error propagation

### SUBTASK 6: Review Remaining Files
- Search entire server package for remaining unwrap():
  ```bash
  rg "\.unwrap\(\)" packages/server/src --type rust
  ```
- Categorize findings:
  - Production code: Replace with ?
  - Test code: Replace with expect()
  - Already handled: Document why safe

### SUBTASK 7: Add Global Clippy Lint
- Add to `packages/server/src/lib.rs`:
  ```rust
  #![deny(clippy::unwrap_used)]
  #![deny(clippy::expect_used)]
  ```
- Allow in test modules:
  ```rust
  #![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
  ```
- Verify no clippy errors remain

## WHAT NEEDS TO CHANGE

**Current State:**
```rust
let db_url = env::var("DATABASE_URL").unwrap_or_default();
let state_key = event.state_key.unwrap();
let user_id = session.user_id.unwrap();
```

**Required State:**
```rust
// unwrap_or_default is OK (doesn't panic):
let db_url = env::var("DATABASE_URL").unwrap_or_default();

// Production code:
let state_key = event.state_key
    .ok_or_else(|| MatrixError::MissingStateKey)?;

let user_id = session.user_id
    .context("Session missing user_id")?;
```

## WHERE CHANGES HAPPEN
- `packages/server/src/config/server_config.rs` (verify safe unwraps)
- `packages/server/src/auth/**/*.rs` (check for unwraps)
- `packages/server/src/_matrix/client/v3/**/*.rs` (endpoint handlers)
- `packages/server/src/_matrix/client/v3/sync/handlers.rs` (sync logic)
- Any other files with remaining unwrap() calls

## WHY CHANGES ARE NEEDED
- Config parsing can fail with invalid values
- Auth operations must handle invalid tokens
- Client requests can have malformed JSON
- State events may be missing required fields
- Sync handlers process untrusted user data
- Production server must never panic

## DEFINITION OF DONE
- [ ] All config unwrap() verified safe or replaced
- [ ] Auth module has proper error handling
- [ ] Client API endpoints return errors, not panic
- [ ] State event handling validates all fields
- [ ] Sync handlers propagate errors correctly
- [ ] No unwrap() remains in production code
- [ ] Clippy lint added to prevent future unwraps
- [ ] No compilation errors or clippy warnings
- [ ] No test code or benchmark code added

## RESEARCH NOTES

### Safe vs Unsafe Unwrap
- Safe: `unwrap_or_default()`, `unwrap_or_else()` - don't panic
- Unsafe: `.unwrap()`, `.expect()` - panic on None/Err
- In production: Never use unsafe unwrap
- In tests: Use expect() with descriptive message

### Option/Result Handling Patterns
```rust
// Option to Result:
value.ok_or_else(|| anyhow!("Missing value"))?

// Result with context:
operation().context("Failed to perform operation")?

// Fallback value:
value.unwrap_or_default()  // Safe - no panic
```

### Authentication Error Handling
- Invalid token: Return 401 Unauthorized
- Expired session: Return M_UNKNOWN_TOKEN
- Missing auth: Return M_MISSING_TOKEN
- Never panic on auth failure

### Matrix Error Responses
- M_FORBIDDEN: User lacks permission
- M_BAD_JSON: Malformed request
- M_MISSING_PARAM: Required field missing
- All return HTTP status + error JSON

## MATRIX SPECIFICATION REFERENCES

**Client-Server API** (`./spec/client/01_foundation_api.md`):
- **Standard Error Codes**: M_FORBIDDEN, M_UNKNOWN_TOKEN, M_MISSING_TOKEN, M_BAD_JSON, M_NOT_JSON, M_NOT_FOUND, M_MISSING_PARAM, etc.
- **Error Response Format**: JSON object with `errcode` and `error` fields (required)
- **HTTP Status Mapping**: Each error code has appropriate HTTP status
- **Request Errors**: M_BAD_JSON, M_NOT_JSON, M_MISSING_PARAM for malformed requests

**Authentication** (`./spec/client/01_foundation_api.md`):
- **Access Tokens**: Opaque strings for authentication
- **Token Validation**: Invalid/expired tokens return M_UNKNOWN_TOKEN
- **Missing Auth**: No token provided returns M_MISSING_TOKEN

**State Event Handling** (`./spec/client/02_rooms_users.md`):
- **State Events**: Must include `state_key` field
- **Required Fields**: event_id, room_id, sender, type, content all required
- **Missing Fields**: Return M_BAD_JSON with descriptive error

**Key Requirements**:
1. All production code must return Result types, never panic
2. Map internal errors to appropriate Matrix error codes
3. Include descriptive error messages for client debugging
4. State events must validate presence of state_key
5. JSON parsing errors return M_BAD_JSON or M_NOT_JSON
6. Authentication failures return correct M_* error codes

## DOCUMENTATION REFERENCES
- Matrix Spec: `./spec/client/01_foundation_api.md` (Error Codes, Authentication)
- Matrix Spec: `./spec/client/02_rooms_users.md` (Rooms, State Events)
- Matrix Error Codes: https://spec.matrix.org/v1.8/client-server-api/#common-error-codes
- Rust Error Handling: https://doc.rust-lang.org/book/ch09-00-error-handling.html
- anyhow Context: https://docs.rs/anyhow/latest/anyhow/trait.Context.html

## CONSTRAINTS
{{ ... }}
- NO benchmark code to be written (separate team handles benchmarks)
- Focus solely on src/ modification and functionality
- Must return proper Matrix error codes
- All endpoint handlers must handle malformed input
