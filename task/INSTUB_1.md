# INSTUB_1: Remove TestCryptoProvider Security Vulnerability

**Priority**: CRITICAL  
**Estimated Effort**: 1 session  
**Category**: Security Fix

---

## OBJECTIVE

Remove `TestCryptoProvider` from production-accessible code paths to eliminate a critical security vulnerability that bypasses all cryptographic signature verification.

**WHY**: `TestCryptoProvider` always returns `true` for signature verification, completely bypassing cryptographic validation. If accessible in production builds, this allows forged events and signatures to be accepted as valid.

---

## BACKGROUND

**Current Location**: [`packages/server/src/security/cross_signing_tests.rs`](../packages/server/src/security/cross_signing_tests.rs):12-13

**The Problem**:
```rust
/// Test helper that always returns true for signature verification.
/// DO NOT use in production - this bypasses all cryptographic validation.
pub struct TestCryptoProvider;
```

This test helper is defined in a file that may be compiled into production builds. Any code that imports and uses this provider completely bypasses signature verification.

**Matrix Spec Requirement**: All event signatures MUST be cryptographically verified per Matrix Federation spec.

---

## SUBTASK 1: Audit TestCryptoProvider Usage

**WHAT**: Find all references to `TestCryptoProvider` in the codebase.

**WHERE**: Search across all `packages/server/src/` (excluding test files)

**HOW**:
```bash
# From workspace root
grep -r "TestCryptoProvider" packages/server/src --exclude="*test*.rs"
```

**EXPECTED**: Should only appear in test files. If found in non-test files, those are security vulnerabilities.

**DEFINITION OF DONE**:
- ✅ All usages of TestCryptoProvider identified
- ✅ List of files that need modification documented

---

## SUBTASK 2: Move TestCryptoProvider to Test-Only Module

**WHAT**: Isolate `TestCryptoProvider` so it's only compiled in test builds.

**WHERE**: [`packages/server/src/security/cross_signing_tests.rs`](../packages/server/src/security/cross_signing_tests.rs)

**HOW**: Wrap the entire file or just the struct in `#[cfg(test)]`:

```rust
#[cfg(test)]
mod test_helpers {
    /// Test helper that always returns true for signature verification.
    /// DO NOT use in production - this bypasses all cryptographic validation.
    pub struct TestCryptoProvider;
    
    #[async_trait::async_trait]
    impl TestCryptoProvider {
        // ... implementation
    }
}

#[cfg(test)]
pub use test_helpers::TestCryptoProvider;
```

**ALTERNATIVE**: If the entire file is test-only, add at the top:
```rust
#![cfg(test)]
```

**DEFINITION OF DONE**:
- ✅ TestCryptoProvider is only compiled in test configuration
- ✅ Production builds cannot access TestCryptoProvider
- ✅ Tests still compile and run successfully

---

## SUBTASK 3: Replace Production Usages with Real Crypto

**WHAT**: Replace any production usage of `TestCryptoProvider` with proper cryptographic verification.

**WHERE**: Any non-test files found in SUBTASK 1

**HOW**: Use the real crypto provider from the repository layer:

```rust
// Replace TestCryptoProvider with:
use crate::repository::crypto_service::CryptoServiceRepository;

// In production code:
let crypto_service = CryptoServiceRepository::new(db.clone());
let valid = crypto_service.verify_signature(/* ... */).await?;
```

**Available Repository**:
- [`packages/surrealdb/src/repository/crypto_service.rs`](../packages/surrealdb/src/repository/crypto_service.rs)
- [`packages/surrealdb/src/repository/crypto.rs`](../packages/surrealdb/src/repository/crypto.rs)

**DEFINITION OF DONE**:
- ✅ All production code uses real cryptographic verification
- ✅ No imports of TestCryptoProvider outside test modules
- ✅ Signature verification actually validates ed25519 signatures

---

## SUBTASK 4: Verify Build and Test

**WHAT**: Ensure production builds don't include test code and tests still pass.

**WHERE**: Workspace root

**HOW**:
```bash
# Build production release (should succeed without test code)
cargo build --release

# Verify tests still work
cargo test --package matryx_server security::cross_signing
```

**CHECK**: Look for any warnings about unused TestCryptoProvider in production builds.

**DEFINITION OF DONE**:
- ✅ Production build compiles successfully
- ✅ Tests compile and pass
- ✅ No compiler warnings about TestCryptoProvider

---

## RESEARCH NOTES

### Rust Test Configuration
- `#[cfg(test)]` - Only compiled when running tests
- `#![cfg(test)]` - Mark entire module/file as test-only
- Test modules are excluded from release builds automatically

### Real Crypto Verification
Location: [`packages/surrealdb/src/repository/crypto_service.rs`](../packages/surrealdb/src/repository/crypto_service.rs)

The real implementation uses:
- `ed25519-dalek` for Ed25519 signature verification
- Proper key management through SurrealDB
- Cryptographic validation per Matrix spec

### Matrix Specification
- **Federation API**: All events must have valid signatures
- **Spec Reference**: [`./spec/server/21-signing-events.md`](../spec/server/21-signing-events.md)
- Events without valid signatures MUST be rejected

---

## DEFINITION OF DONE

**Task complete when**:
- ✅ TestCryptoProvider is only accessible in test builds (`#[cfg(test)]`)
- ✅ No production code imports or uses TestCryptoProvider
- ✅ All signature verification uses real cryptographic validation
- ✅ Production release builds successfully
- ✅ All tests still pass

**NO REQUIREMENTS FOR**:
- ❌ New unit tests (existing tests must still pass)
- ❌ Integration tests
- ❌ Benchmarks
- ❌ Documentation beyond code comments

---

## RELATED FILES

- [`packages/server/src/security/cross_signing_tests.rs`](../packages/server/src/security/cross_signing_tests.rs) - Contains TestCryptoProvider
- [`packages/surrealdb/src/repository/crypto_service.rs`](../packages/surrealdb/src/repository/crypto_service.rs) - Real crypto implementation
- [`packages/surrealdb/src/repository/crypto.rs`](../packages/surrealdb/src/repository/crypto.rs) - Crypto primitives
- [`./spec/server/21-signing-events.md`](../spec/server/21-signing-events.md) - Matrix signing specification
