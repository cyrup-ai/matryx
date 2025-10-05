# STUB_13: Third-Party Invites - Placeholder Values Fix

## OBJECTIVE
Fix hardcoded placeholder values in third-party invite event creation to use real identity server data.

## CORE PROBLEM

The third-party invite implementation at `packages/server/src/_matrix/client/v3/create_room.rs` contains critical placeholder values that prevent proper functionality:

**Lines 120-124 contain:**
```rust
"key_validity_url": format!("https://{}/_matrix/identity/api/v1/pubkey/isvalid", "identity.server"), // ❌ Hardcoded
"public_key": "public_key_placeholder", // ❌ Placeholder
"public_keys": [], // ❌ Empty
```

**Root Cause:** The `create_third_party_invite_event()` function doesn't receive the `id_server` parameter, forcing the use of placeholder values.

## COMPLETED ITEMS (No Action Required)

- ✅ **Power Level Updates** - Full implementation complete with validation and state events
- ✅ **Event Validation** - Comprehensive validation with all 7 required checks implemented
- ✅ **Mention Processing Integration** - Fully integrated in message send endpoint

## REQUIRED FIXES

### Fix 1: Add `id_server` Parameter to Function Signature

**File:** `packages/server/src/_matrix/client/v3/create_room.rs`  
**Line:** 110

**Current:**
```rust
async fn create_third_party_invite_event(
    event_repo: &EventRepository,
    room_id: &str,
    sender: &str,
    display_name: &str,
    signed: &SignedThirdPartyInvite,
) -> Result<Event, RepositoryError>
```

**Change to:**
```rust
async fn create_third_party_invite_event(
    event_repo: &EventRepository,
    room_id: &str,
    sender: &str,
    display_name: &str,
    signed: &SignedThirdPartyInvite,
    id_server: &str,  // ✅ ADD THIS PARAMETER
) -> Result<Event, RepositoryError>
```

### Fix 2: Replace Placeholder Values with Real Data

**File:** `packages/server/src/_matrix/client/v3/create_room.rs`  
**Lines:** 120-124

**Current:**
```rust
let content = serde_json::json!({
    "display_name": display_name,
    "key_validity_url": format!("https://{}/_matrix/identity/api/v1/pubkey/isvalid", "identity.server"), // ❌
    "public_key": "public_key_placeholder", // ❌
    "public_keys": [], // ❌
});
```

**Change to:**
```rust
let content = serde_json::json!({
    "display_name": display_name,
    "key_validity_url": format!("https://{}/_matrix/identity/api/v1/pubkey/isvalid", id_server), // ✅ Use id_server parameter
    "public_key": signed.signatures.values().next()
        .and_then(|sigs| sigs.keys().next())
        .map(|k| k.as_str())
        .unwrap_or(""), // ✅ Extract from signatures
    "public_keys": signed.signatures.iter()
        .flat_map(|(_, sigs)| sigs.keys())
        .map(|k| k.as_str())
        .collect::<Vec<_>>(), // ✅ Extract all public keys from signatures
});
```

### Fix 3: Pass `id_server` to Function Call

**File:** `packages/server/src/_matrix/client/v3/create_room.rs`  
**Line:** 288

**Current:**
```rust
if let Err(e) = create_third_party_invite_event(
    &event_repo,
    &room.room_id,
    &user_id,
    display_name,
    &signed_invite,
    // ❌ Missing id_server
)
```

**Change to:**
```rust
if let Err(e) = create_third_party_invite_event(
    &event_repo,
    &room.room_id,
    &user_id,
    display_name,
    &signed_invite,
    id_server, // ✅ ADD THIS ARGUMENT
)
```

## UNDERSTANDING THE SIGNED DATA STRUCTURE

The `SignedThirdPartyInvite` type (from `packages/entity/src/types/signed_third_party_invite.rs`):

```rust
pub struct SignedThirdPartyInvite {
    pub mxid: String,
    pub signatures: HashMap<String, HashMap<String, String>>,
    pub token: String,
}
```

**Structure:**
- `signatures`: `HashMap<server_name, HashMap<key_id, signature>>`
  - Outer map: Identity server domain → signing keys
  - Inner map: Key ID (e.g., "ed25519:1") → Base64 signature

**Example:**
```json
{
  "mxid": "@user:example.com",
  "signatures": {
    "id.example.com": {
      "ed25519:1": "Base64SignatureHere...",
      "ed25519:2": "AnotherBase64Sig..."
    }
  },
  "token": "RandomTokenString"
}
```

**Extraction Logic:**
- **key_validity_url**: Use the `id_server` parameter passed in
- **public_key**: Extract first key ID from signatures map (e.g., "ed25519:1")
- **public_keys**: Collect all key IDs from all servers in the signatures map

---

## DEFINITION OF DONE

- [ ] `id_server` parameter added to `create_third_party_invite_event()` function signature
- [ ] `id_server` passed to function call at line 288
- [ ] `key_validity_url` uses `id_server` parameter instead of hardcoded string
- [ ] `public_key` extracts real key ID from `signed.signatures`
- [ ] `public_keys` array populated with all key IDs from `signed.signatures`
- [ ] No placeholder or hardcoded values remain in third-party invite event creation
- [ ] Code compiles without errors
- [ ] Third-party invite events contain valid Matrix-compliant data
