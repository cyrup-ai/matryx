# TPBRIDGE_1: Fix Third-Party Bridge Protocol Field Types

**Status**: Ready for Implementation  
**Priority**: CRITICAL  
**Estimated Effort**: 1-2 weeks  
**Package**: packages/server, packages/surrealdb

---

## OBJECTIVE

Fix incomplete third-party bridge protocol field type handling to prevent data loss and ensure Matrix Application Service bridges (IRC, Slack, Discord, etc.) function correctly.

---

## ROOT CAUSE ANALYSIS

The `FieldType` struct is missing the `name` field which serves as the identifier for protocol fields. This causes the implementation to incorrectly use `placeholder` (a UI example value) as the field identifier throughout the codebase.

### Matrix Specification Reference

**Official Spec Location**: [./tmp/matrix-spec/data/api/application-service/definitions/protocol_base.yaml](../tmp/matrix-spec/data/api/application-service/definitions/protocol_base.yaml)

Per the Matrix Application Service API specification, a Protocol has:

```yaml
user_fields:
  description: Fields which may be used to identify a third-party user
  type: array
  items:
    type: string  # ← Field NAMES as strings (e.g., "network", "nickname")

location_fields:
  description: Fields which may be used to identify a third-party location
  type: array
  items:
    type: string  # ← Field NAMES as strings (e.g., "network", "channel")

field_types:
  description: Type definitions for fields defined in user_fields and location_fields
  type: object
  additionalProperties:
    title: Field Type
    type: object
    properties:
      regexp:
        type: string
        description: Regular expression for validation of field's value
      placeholder:
        type: string
        description: A placeholder serving as a valid example of the field value
    required: ['regexp', 'placeholder']
```

**Example from Spec** ([./tmp/matrix-spec/data/api/client-server/third_party_lookup.yaml](../tmp/matrix-spec/data/api/client-server/third_party_lookup.yaml)):

```json
{
  "irc": {
    "user_fields": ["network", "nickname"],
    "location_fields": ["network", "channel"],
    "field_types": {
      "network": {
        "regexp": "([a-z0-9]+\\.)*[a-z0-9]+",
        "placeholder": "irc.example.org"
      },
      "nickname": {
        "regexp": "[^\\s]+",
        "placeholder": "username"
      },
      "channel": {
        "regexp": "#[^\\s]+",
        "placeholder": "#foobar"
      }
    }
  }
}
```

**Key Insight**: 
- Field names are identifiers: `"network"`, `"nickname"`, `"channel"`
- Placeholders are UI hints: `"irc.example.org"`, `"username"`, `"#foobar"`
- The HashMap key is the field NAME, not the placeholder value

### Current Implementation Issues

**Issue 1: FieldType Missing Name Field**

File: [`packages/surrealdb/src/repository/third_party.rs:73-77`](../packages/surrealdb/src/repository/third_party.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldType {
    pub regexp: String,
    pub placeholder: String,  // ✗ Missing: name field!
}
```

**Issue 2: Wrong HashMap Key**

File: [`packages/server/src/_matrix/client/v3/thirdparty/protocol/by_protocol.rs:89-96`](../packages/server/src/_matrix/client/v3/thirdparty/protocol/by_protocol.rs)

```rust
let mut field_types: HashMap<String, FieldType> = HashMap::new();
for field in &protocol_config.user_fields {
    field_types.insert(
        format!("user.{}", field.placeholder),  // ✗ WRONG: using placeholder as key!
        FieldType {                              // ✗ WRONG: "user." prefix not per spec!
            regexp: field.regexp.clone(),
            placeholder: field.placeholder.clone(),
        },
    );
}
```

If `field.placeholder = "irc.example.org"`, this creates:
- Key: `"user.irc.example.org"` ✗ **WRONG**
- Should be: `"network"` ✓ **CORRECT**

**Issue 3: Wrong Field Validation**

File: [`packages/surrealdb/src/repository/third_party_service.rs:47-48`](../packages/surrealdb/src/repository/third_party_service.rs)

```rust
let field_exists = protocol_config.location_fields
    .iter()
    .any(|f| f.placeholder == *field_name);  // ✗ WRONG: comparing placeholder to field name!
```

This compares UI hint (`"irc.example.org"`) to field identifier (`"network"`), which never matches.

---

## IMPLEMENTATION PLAN

### STEP 1: Add Name Field to FieldType Struct

**File**: `packages/surrealdb/src/repository/third_party.rs`  
**Lines**: 73-77

**Current Code**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldType {
    pub regexp: String,
    pub placeholder: String,
}
```

**Required Change**:
```rust
/// Field type definition for third-party protocol fields per Matrix spec.
///
/// Defines validation and UI hints for protocol field identifiers.
///
/// **Matrix Specification**: Application Service API - Third Party Protocol Metadata
/// See: tmp/matrix-spec/data/api/application-service/definitions/protocol_base.yaml
///
/// # Example
///
/// For an IRC bridge, a "network" field might be defined as:
/// ```rust
/// FieldType {
///     name: "network".to_string(),              // Field identifier
///     regexp: "([a-z0-9]+\\.)*[a-z0-9]+".to_string(),  // Validation pattern
///     placeholder: "irc.example.org".to_string() // UI example text
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldType {
    /// Unique identifier for this field (e.g., "network", "nickname", "channel")
    /// This is used as the key in field_types HashMap and referenced in user_fields/location_fields arrays
    pub name: String,
    
    /// Regular expression for validating field values
    /// Example: "^#[a-z0-9_-]+$" for IRC channels
    pub regexp: String,
    
    /// Placeholder text shown to users in client UI
    /// Example: "#channel" for IRC channel field, "irc.example.org" for network field
    pub placeholder: String,
}
```

### STEP 2: Fix field_types HashMap Insertion

**File**: `packages/server/src/_matrix/client/v3/thirdparty/protocol/by_protocol.rs`  
**Lines**: 89-106

**Current Broken Code**:
```rust
// Build field_types map
let mut field_types: HashMap<String, FieldType> = HashMap::new();
for field in &protocol_config.user_fields {
    field_types.insert(
        format!("user.{}", field.placeholder),  // ✗ WRONG KEY
        FieldType {
            regexp: field.regexp.clone(),
            placeholder: field.placeholder.clone(),
        },
    );
}
for field in &protocol_config.location_fields {
    field_types.insert(
        format!("location.{}", field.placeholder),  // ✗ WRONG KEY
        FieldType {
            regexp: field.regexp.clone(),
            placeholder: field.placeholder.clone(),
        },
    );
}
```

**Correct Implementation**:
```rust
// Build field_types map per Matrix spec
// Keys are field names (e.g., "network", "nickname", "channel")
let mut field_types: HashMap<String, FieldType> = HashMap::new();

// Add user field definitions
for field in &protocol_config.user_fields {
    field_types.insert(
        field.name.clone(),  // ✓ Use field name as key (not placeholder!)
        FieldType {
            regexp: field.regexp.clone(),
            placeholder: field.placeholder.clone(),
        },
    );
}

// Add location field definitions
for field in &protocol_config.location_fields {
    field_types.insert(
        field.name.clone(),  // ✓ Use field name as key (not placeholder!)
        FieldType {
            regexp: field.regexp.clone(),
            placeholder: field.placeholder.clone(),
        },
    );
}
```

### STEP 3: Fix Response Field Arrays

**File**: `packages/server/src/_matrix/client/v3/thirdparty/protocol/by_protocol.rs`  
**Lines**: 128-137

**Current Code**:
```rust
user_fields: protocol_config
    .user_fields
    .into_iter()
    .map(|f| FieldType { regexp: f.regexp, placeholder: f.placeholder })
    .collect(),
```

**Correct Implementation**:
```rust
// Per Matrix spec, user_fields should be array of field NAMES (strings)
// NOT FieldType objects
user_fields: protocol_config
    .user_fields
    .into_iter()
    .map(|f| f.name)  // ✓ Extract field name only
    .collect(),

location_fields: protocol_config
    .location_fields
    .into_iter()
    .map(|f| f.name)  // ✓ Extract field name only
    .collect(),
```

But wait - looking at the server's FieldType definition, it expects the same structure. So we need to update the response struct.

**File**: `packages/server/src/_matrix/client/v3/thirdparty/protocol/by_protocol.rs`  
**Lines**: 29-30

**Current Code**:
```rust
pub struct ProtocolResponse {
    pub user_fields: Vec<FieldType>,      // ✗ WRONG: should be Vec<String>
    pub location_fields: Vec<FieldType>,  // ✗ WRONG: should be Vec<String>
    pub icon: Option<String>,
    pub field_types: HashMap<String, FieldType>,
    pub instances: Vec<ProtocolInstance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_status: Option<String>,
}
```

**Correct Implementation**:
```rust
pub struct ProtocolResponse {
    pub user_fields: Vec<String>,         // ✓ Array of field names
    pub location_fields: Vec<String>,     // ✓ Array of field names
    pub icon: Option<String>,
    pub field_types: HashMap<String, FieldType>,
    pub instances: Vec<ProtocolInstance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bridge_status: Option<String>,
}
```

Then update the response building:
```rust
let response = ProtocolResponse {
    user_fields: protocol_config
        .user_fields
        .iter()
        .map(|f| f.name.clone())
        .collect(),
    location_fields: protocol_config
        .location_fields
        .iter()
        .map(|f| f.name.clone())
        .collect(),
    icon: protocol_config.avatar_url,
    field_types,  // Already correctly built above
    instances,
    bridge_status,
};
```

### STEP 4: Fix protocols.rs Endpoint

**File**: `packages/server/src/_matrix/client/v3/thirdparty/protocols.rs`  
**Lines**: 17-20, 77-107

**Update Protocol Struct**:
```rust
#[derive(Serialize, Deserialize)]
pub struct Protocol {
    pub user_fields: Vec<String>,         // ✓ Changed from Vec<FieldType>
    pub location_fields: Vec<String>,     // ✓ Changed from Vec<FieldType>
    pub icon: String,
    pub field_types: HashMap<String, FieldType>,
    pub instances: Vec<ProtocolInstance>,
}
```

**Update Conversion Logic** (lines 77-107):
```rust
for (protocol_id, protocol_config) in protocols_map {
    // Extract field names for user_fields and location_fields
    let user_fields: Vec<String> = protocol_config
        .user_fields
        .iter()
        .map(|f| f.name.clone())  // ✓ Extract field name
        .collect();

    let location_fields: Vec<String> = protocol_config
        .location_fields
        .iter()
        .map(|f| f.name.clone())  // ✓ Extract field name
        .collect();

    // Build field_types HashMap with field names as keys
    let mut field_types: HashMap<String, FieldType> = HashMap::new();
    
    for field in &protocol_config.user_fields {
        field_types.insert(
            field.name.clone(),  // ✓ Use field name as key
            FieldType {
                regexp: field.regexp.clone(),
                placeholder: field.placeholder.clone(),
            },
        );
    }
    
    for field in &protocol_config.location_fields {
        field_types.insert(
            field.name.clone(),  // ✓ Use field name as key
            FieldType {
                regexp: field.regexp.clone(),
                placeholder: field.placeholder.clone(),
            },
        );
    }

    let instances: Vec<ProtocolInstance> = protocol_config
        .instances
        .into_iter()
        .map(|i| ProtocolInstance {
            desc: i.desc,
            icon: i.icon,
            fields: i.fields,
            network_id: i.network_id,
        })
        .collect();

    let protocol = Protocol {
        user_fields,
        location_fields,
        icon: protocol_config.avatar_url.unwrap_or_else(|| "mxc://".to_string()),
        field_types,
        instances,
    };

    response.insert(protocol_id, protocol);
}
```

### STEP 5: Fix Validation Logic in Service Layer

**File**: `packages/surrealdb/src/repository/third_party_service.rs`  
**Lines**: 47-48, 77-78, 300-301

**Current Broken Validation**:
```rust
let field_exists = protocol_config.location_fields
    .iter()
    .any(|f| f.placeholder == *field_name);  // ✗ WRONG: comparing placeholder to name!
```

**Correct Validation**:
```rust
let field_exists = protocol_config.location_fields
    .iter()
    .any(|f| f.name == *field_name);  // ✓ Compare field name to field name
```

**Apply to all occurrences**:

Line 47-48:
```rust
let field_exists = protocol_config.location_fields
    .iter()
    .any(|f| f.name == *field_name);
```

Line 77-78:
```rust
let field_exists = protocol_config.user_fields
    .iter()
    .any(|f| f.name == *field_name);
```

Line 300-301:
```rust
for field_def in field_definitions {
    if !fields.contains_key(&field_def.name) {  // ✓ Use field.name
        return Err(RepositoryError::ValidationError {
            field: field_def.name.clone(),  // ✓ Use field.name
            message: format!("Required field '{}' is missing", field_def.name),
        });
    }

    if let Some(field_value) = fields.get(&field_def.name) {  // ✓ Use field.name
        // ... validation logic
    }
}
```

---

## EXAMPLE: IRC Bridge Protocol Configuration

After implementation, an IRC bridge protocol should be structured as:

```rust
ThirdPartyProtocol {
    protocol_id: "irc".to_string(),
    display_name: "IRC Bridge".to_string(),
    avatar_url: Some("mxc://example.org/aBcDeFgH".to_string()),
    
    // user_fields contains FieldType objects with names
    user_fields: vec![
        FieldType {
            name: "network".to_string(),
            regexp: "([a-z0-9]+\\.)*[a-z0-9]+".to_string(),
            placeholder: "irc.example.org".to_string(),
        },
        FieldType {
            name: "nickname".to_string(),
            regexp: "[^\\s]+".to_string(),
            placeholder: "username".to_string(),
        },
    ],
    
    // location_fields contains FieldType objects with names
    location_fields: vec![
        FieldType {
            name: "network".to_string(),
            regexp: "([a-z0-9]+\\.)*[a-z0-9]+".to_string(),
            placeholder: "irc.example.org".to_string(),
        },
        FieldType {
            name: "channel".to_string(),
            regexp: "#[^\\s]+".to_string(),
            placeholder: "#foobar".to_string(),
        },
    ],
    
    instances: vec![/* ... */],
}
```

And the JSON response would be:

```json
{
  "irc": {
    "user_fields": ["network", "nickname"],
    "location_fields": ["network", "channel"],
    "icon": "mxc://example.org/aBcDeFgH",
    "field_types": {
      "network": {
        "regexp": "([a-z0-9]+\\.)*[a-z0-9]+",
        "placeholder": "irc.example.org"
      },
      "nickname": {
        "regexp": "[^\\s]+",
        "placeholder": "username"
      },
      "channel": {
        "regexp": "#[^\\s]+",
        "placeholder": "#foobar"
      }
    },
    "instances": [/* ... */]
  }
}
```

---

## SUMMARY OF CHANGES

### Files to Modify

1. **`packages/surrealdb/src/repository/third_party.rs`**
   - Add `name: String` field to `FieldType` struct (line 73)
   - Add comprehensive documentation to `FieldType`

2. **`packages/server/src/_matrix/client/v3/thirdparty/protocol/by_protocol.rs`**
   - Change `ProtocolResponse.user_fields` from `Vec<FieldType>` to `Vec<String>` (line 29)
   - Change `ProtocolResponse.location_fields` from `Vec<FieldType>` to `Vec<String>` (line 30)
   - Fix `field_types.insert()` to use `field.name` instead of `field.placeholder` (lines 91, 99)
   - Remove "user." and "location." prefixes from HashMap keys (lines 91, 99)
   - Update response building to extract field names (lines 128-137)

3. **`packages/server/src/_matrix/client/v3/thirdparty/protocols.rs`**
   - Change `Protocol.user_fields` from `Vec<FieldType>` to `Vec<String>` (line 18)
   - Change `Protocol.location_fields` from `Vec<FieldType>` to `Vec<String>` (line 19)
   - Fix field_types HashMap building to use `field.name` as keys (lines 101-107)
   - Update conversion logic to extract field names (lines 77-87)

4. **`packages/surrealdb/src/repository/third_party_service.rs`**
   - Change validation from `f.placeholder` to `f.name` (line 48)
   - Change validation from `f.placeholder` to `f.name` (line 78)
   - Change validation from `field_def.placeholder` to `field_def.name` (lines 300-316)

### Matrix Spec References

- **Application Service API**: [./tmp/matrix-spec/content/application-service-api.md](../tmp/matrix-spec/content/application-service-api.md)
- **Protocol Base Schema**: [./tmp/matrix-spec/data/api/application-service/definitions/protocol_base.yaml](../tmp/matrix-spec/data/api/application-service/definitions/protocol_base.yaml)
- **Third Party Lookup API**: [./tmp/matrix-spec/data/api/client-server/third_party_lookup.yaml](../tmp/matrix-spec/data/api/client-server/third_party_lookup.yaml)

---

## DEFINITION OF DONE

- [ ] `FieldType` struct has `name: String` field added
- [ ] `FieldType` struct has comprehensive documentation with examples
- [ ] `field_types` HashMap uses `field.name` as keys (not `field.placeholder`)
- [ ] No "user." or "location." prefixes on field_types HashMap keys
- [ ] `user_fields` and `location_fields` in responses are `Vec<String>` (field names only)
- [ ] Validation logic in service layer uses `field.name` for comparisons
- [ ] All affected files modified consistently
- [ ] Code compiles without errors
- [ ] JSON responses match Matrix specification exactly

---

## NOTES

- This is a **data modeling bug** where the wrong field (`placeholder`) is being used as an identifier
- The bug affects ALL third-party bridge operations: IRC, Slack, Discord, etc.
- Without this fix, bridges cannot correctly validate or route third-party identifiers
- The `placeholder` field is for UI display only, not for identification
- Field names are stable identifiers used throughout the protocol lifecycle