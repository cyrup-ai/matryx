# TPBRIDGE_1: Fix Third-Party Bridge Protocol Field Types

**Status**: Ready for Implementation
**Priority**: CRITICAL
**Estimated Effort**: 1-2 weeks
**Package**: packages/server, packages/surrealdb

---

## OBJECTIVE

Fix incomplete third-party bridge protocol field type handling to prevent data loss and ensure Matrix Application Service bridges (IRC, Slack, Discord, etc.) function correctly.

---

## PROBLEM DESCRIPTION

The `FieldType` struct and related field type operations have incomplete implementations:
1. `FieldType.placeholder` field purpose is unclear and potentially misnamed
2. Map insert operations are missing the proper key (field name)
3. No validation of field types against Matrix specification

This causes:
- Bridge protocol metadata to be incorrectly structured
- Field type mapping to be incomplete
- Third-party protocol discovery to return malformed data
- Matrix bridges unable to function

---

## RESEARCH NOTES

**Matrix Specification Reference**:
- Section: Application Service API - Third Party Lookups
- Endpoint: `GET /_matrix/client/v3/thirdparty/protocol/{protocol}`
- FieldTypes object structure per spec

**Current Implementation Issues**:

File: `packages/surrealdb/src/repository/third_party.rs:73-77`
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldType {
    pub regexp: String,
    pub placeholder: String,  // ← Purpose unclear
}
```

File: `packages/server/src/_matrix/client/v3/thirdparty/protocol/by_protocol.rs:90-96`
```rust
field_types.insert(
    // field.name.clone(),  ← MISSING KEY!
    FieldType {
        regexp: field.regexp.clone(),
        placeholder: field.placeholder.clone(),
    },
);
```

---

## SUBTASK 1: Verify Matrix Specification Compliance

**Objective**: Confirm the correct structure for FieldType per Matrix spec.

**Actions**:
1. Review Matrix Client-Server API specification for third-party protocol lookups
2. Identify the exact structure required for `FieldTypes` objects
3. Confirm whether `placeholder` is a legitimate field per spec or misnamed
4. Document findings in code comments

**Files to Review**:
- Matrix spec: https://spec.matrix.org/v1.9/application-service-api/
- Matrix spec section on third-party protocol metadata

**Definition of Done**:
- Clear understanding of Matrix spec requirements for FieldType
- Documentation comment added to FieldType struct explaining its purpose

---

## SUBTASK 2: Fix FieldType Structure in Repository Layer

**Objective**: Correct the FieldType struct definition in the repository layer.

**Location**: `packages/surrealdb/src/repository/third_party.rs`

**Changes Required**:

1. Add comprehensive documentation to FieldType struct:
```rust
/// Field type definition for third-party protocol fields per Matrix spec
///
/// This represents a field that users need to provide when looking up
/// third-party users or locations (e.g., IRC channel, Slack workspace).
///
/// **Matrix Specification**: Application Service API - Third Party Lookups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldType {
    /// Regular expression for validating field values
    /// Example: "^#[a-z0-9_-]+$" for IRC channels
    pub regexp: String,

    /// Placeholder text shown to users in client UI
    /// Example: "#channel" for IRC channel field
    pub placeholder: String,
}
```

2. If Matrix spec does NOT include `placeholder`, rename or restructure accordingly

3. Add validation method:
```rust
impl FieldType {
    /// Validate that this FieldType is correctly formed
    pub fn validate(&self) -> Result<(), ValidationError> {
        // Validate regexp compiles
        regex::Regex::new(&self.regexp)
            .map_err(|e| ValidationError::InvalidRegexp(e.to_string()))?;

        // Ensure placeholder is not empty
        if self.placeholder.trim().is_empty() {
            return Err(ValidationError::EmptyPlaceholder);
        }

        Ok(())
    }
}
```

**Files to Modify**:
- `packages/surrealdb/src/repository/third_party.rs`

**Definition of Done**:
- FieldType struct has complete documentation
- Validation method added and tested
- Field names match Matrix specification exactly

---

## SUBTASK 3: Fix Field Type Map Insertion Logic

**Objective**: Correct the field_types HashMap insertion operations to use proper keys.

**Locations**:
- `packages/server/src/_matrix/client/v3/thirdparty/protocol/by_protocol.rs`
- `packages/server/src/_matrix/client/v3/thirdparty/protocols.rs`

**Current Broken Code** (by_protocol.rs:89-96):
```rust
let mut field_types: HashMap<String, FieldType> = HashMap::new();
for field in &protocol_config.user_fields {
    field_types.insert(
        field.placeholder.clone(),  // ← WRONG: using placeholder as key!
        FieldType {
            regexp: field.regexp.clone(),
            placeholder: field.placeholder.clone(),
        },
    );
}
```

**Required Fix**:

Determine the correct key source:
- Option A: If FieldType needs a `name` field, add it to the struct
- Option B: If protocol_config.user_fields has a name, use it
- Option C: If the field name comes from elsewhere, identify source

**Correct Implementation** (assuming Option A):
```rust
let mut field_types: HashMap<String, FieldType> = HashMap::new();
for field in &protocol_config.user_fields {
    field_types.insert(
        field.name.clone(),  // ← Use actual field name as key
        FieldType {
            regexp: field.regexp.clone(),
            placeholder: field.placeholder.clone(),
        },
    );
}

// Repeat for location_fields
for field in &protocol_config.location_fields {
    field_types.insert(
        field.name.clone(),
        FieldType {
            regexp: field.regexp.clone(),
            placeholder: field.placeholder.clone(),
        },
    );
}
```

**Files to Modify**:
- `packages/server/src/_matrix/client/v3/thirdparty/protocol/by_protocol.rs` (lines 89-106)
- `packages/server/src/_matrix/client/v3/thirdparty/protocols.rs` (lines 101-107)

**Definition of Done**:
- field_types HashMap uses correct keys (not placeholder values)
- Both user_fields and location_fields properly inserted
- Logic matches Matrix specification requirements

---

## SUBTASK 4: Add Field Type Conversion Logic

**Objective**: Ensure proper conversion between repository types and server types.

**Location**: `packages/server/src/_matrix/client/v3/thirdparty/protocols.rs`

**Current Code** (lines 77-87):
```rust
let user_fields: Vec<FieldType> = protocol_config
    .user_fields
    .into_iter()
    .map(|f| /* MISSING CONVERSION LOGIC */)
    .collect();
```

**Required Implementation**:
```rust
let user_fields: Vec<String> = protocol_config
    .user_fields
    .into_iter()
    .map(|f| f.name.clone())
    .collect();

let location_fields: Vec<String> = protocol_config
    .location_fields
    .into_iter()
    .map(|f| f.name.clone())
    .collect();
```

**Files to Modify**:
- `packages/server/src/_matrix/client/v3/thirdparty/protocols.rs` (lines 77-87)
- `packages/server/src/_matrix/client/v3/thirdparty/protocol/by_protocol.rs` (lines 128-137)

**Definition of Done**:
- Conversion logic properly extracts field names
- Both user_fields and location_fields correctly converted
- Response structure matches Matrix specification

---

## CONSTRAINTS

⚠️ **NO TESTS**: Do not write unit tests, integration tests, or test fixtures. Test team handles all testing.

⚠️ **NO BENCHMARKS**: Do not write benchmark code. Performance team handles benchmarking.

⚠️ **FOCUS ON FUNCTIONALITY**: Only modify production code in ./src directories.

---

## DEPENDENCIES

**Matrix Specification**:
- Clone: https://github.com/matrix-org/matrix-spec
- Section: Application Service API
- Focus: Third Party Lookups

**Validation**:
- Requires `regex` crate for validation (already in dependencies)
- May need to add ValidationError type if not present

---

## DEFINITION OF DONE

- [ ] FieldType struct matches Matrix specification exactly
- [ ] FieldType has comprehensive documentation explaining each field
- [ ] Validation method added to FieldType
- [ ] field_types HashMap insertion uses correct keys (not placeholder values)
- [ ] Conversion logic properly maps between repository and server types
- [ ] All affected files updated consistently
- [ ] No compilation errors
- [ ] No test code written
- [ ] No benchmark code written

---

## FILES TO MODIFY

1. `packages/surrealdb/src/repository/third_party.rs` (lines 73-77)
2. `packages/server/src/_matrix/client/v3/thirdparty/protocol/by_protocol.rs` (lines 89-141)
3. `packages/server/src/_matrix/client/v3/thirdparty/protocols.rs` (lines 77-107)

---

## NOTES

- This is a data integrity issue - incorrect implementation leads to malformed API responses
- Matrix bridges depend on this for protocol discovery and configuration
- Field names are critical for client UI generation
- `placeholder` field (if per spec) is for UI hint text, NOT a generic placeholder marker
