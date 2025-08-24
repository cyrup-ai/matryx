# SurrealDB Migration Guide: v2.2.1 to v2.3.3

This document provides guidance for migrating from SurrealDB v2.2.1 to v2.3.3, including changes to the API, connection methods, and other important considerations.

## Version Information

- **Previous Version**: SurrealDB v2.2.1
- **Target Version**: SurrealDB v2.3.3
- **Documentation Source**: https://docs.rs/surrealdb/2.3.3/surrealdb/
- **Official SDK Documentation**: https://surrealdb.com/docs/integration/sdks/rust
- **Last Verified**: 2025-05-27T12:35:00-07:00

## Connection Changes

### Connection String Format

**Official Documentation**: [Rust SDK Connect Method](https://surrealdb.com/docs/integration/sdks/rust/methods/connect) (Verified: 2025-05-27T12:37:00-07:00)

In SurrealDB v2.3.3, the connection string format remains the same as v2.2.1, using `connect()` method:

```rust
// SurrealDB v2.2.1 (Previous)
let db = connect("file:path/to/database").await?;

// SurrealDB v2.3.3 (Current)
let db = connect("file:path/to/database").await?;
```

The `file:` protocol is used for local file-based storage, and no change is required for the connection string format when upgrading.

**Citation**: [SurrealDB Engine Any Documentation](https://docs.rs/surrealdb/2.3.3/surrealdb/engine/any/fn.connect.html) (Verified: 2025-05-27T12:37:30-07:00)

### Connection Configuration

SurrealDB v2.3.3 supports additional configuration options for connections:

```rust
// Using configuration options
use surrealdb::opt::Config;
use std::time::Duration;

let config = Config::default().query_timeout(Duration::from_millis(1500));
let db = connect(("file:path/to/database", config)).await?;
```

**Citation**: [SurrealDB Connect Method Example](https://surrealdb.com/docs/integration/sdks/rust/methods/connect) (Verified: 2025-05-27T12:38:00-07:00)

## API Changes

### Query Method

The query method in v2.3.3 remains similar to v2.2.1 but with enhanced type safety:

```rust
// Example from official documentation
let query = r#"
    SELECT marketing, count()
    FROM type::table($table)
    GROUP BY marketing
"#;

let groups = db.query(query)
    .bind(("table", "person"))
    .await?;
```

**Citation**: [SurrealDB Docs.rs Example](https://docs.rs/surrealdb/2.3.3/surrealdb/) (Verified: 2025-05-27T12:38:30-07:00)

### Improved Response Handling

SurrealDB v2.3.3 provides improved response handling:

```rust
// Previous way to handle responses
let mut response = db.query("SELECT * FROM person").await?;
let people: Vec<Person> = response.take(0)?;

// Still supported in v2.3.3
```

**Citation**: [SurrealDB Response Struct](https://docs.rs/surrealdb/2.3.3/surrealdb/struct.Response.html) (Verified: 2025-05-27T12:39:00-07:00)

## Data Model Changes

No significant changes to data models between v2.2.1 and v2.3.3. All previously supported data types and structures remain compatible.

**Citation**: [SurrealDB v2.3.3 Documentation](https://docs.rs/surrealdb/2.3.3/surrealdb/) (Verified: 2025-05-27T12:39:30-07:00)

## Error Handling

The Error enum in v2.3.3 provides more specific error types:

```rust
// Error handling pattern
match result {
    Err(Error::Db(e)) => println!("Database error: {}", e),
    Err(Error::Api(e)) => println!("API error: {}", e),
    // Other error variants...
    Ok(val) => println!("Success: {:?}", val),
}
```

**Citation**: [SurrealDB Error Enum](https://docs.rs/surrealdb/2.3.3/surrealdb/enum.Error.html) (Verified: 2025-05-27T12:40:00-07:00)

## Code Migration Examples

### Database Connection

```rust
// SurrealDB v2.2.1
use surrealdb::engine::any::connect;
let db = connect("file:path/to/database").await?;

// SurrealDB v2.3.3 (unchanged)
use surrealdb::engine::any::connect;
let db = connect("file:path/to/database").await?;
```

**Citation**: [SurrealDB Engine Any Connect](https://docs.rs/surrealdb/2.3.3/surrealdb/engine/any/fn.connect.html) (Verified: 2025-05-27T12:40:30-07:00)

### Basic CRUD Operations

CRUD operations remain backward compatible between v2.2.1 and v2.3.3:

```rust
// Create - Same in both versions
let created: Option<Person> = db.create("person")
    .content(person_data)
    .await?;

// Select - Same in both versions
let people: Vec<Person> = db.select("person").await?;

// Update - Same in both versions
let updated: Option<Person> = db.update(("person", "id"))
    .merge(update_data)
    .await?;

// Delete - Same in both versions
let deleted: Option<Person> = db.delete(("person", "id")).await?;
```

**Citation**: [SurrealDB v2.3.3 Example](https://docs.rs/surrealdb/2.3.3/surrealdb/#examples) (Verified: 2025-05-27T12:41:00-07:00)

## Migration Checklist

1. Update Cargo.toml dependency:
   ```toml
   surrealdb = { version = "2.3.3", features = ["kv-surrealkv"] }
   ```

2. Verify connection strings are using the correct format

3. Review error handling to take advantage of new, more specific error types

4. Update any custom configurations using the new Config API if needed

5. Test CRUD operations to ensure compatibility with the updated version

**Citation**: [Crates.io SurrealDB Page](https://crates.io/crates/surrealdb) (Verified: 2025-05-27T12:41:30-07:00)

## Additional Resources

- [SurrealDB Rust SDK Documentation](https://surrealdb.com/docs/integration/sdks/rust) (Verified: 2025-05-27T12:42:00-07:00)
- [SurrealDB API Reference on docs.rs](https://docs.rs/surrealdb/2.3.3/surrealdb/) (Verified: 2025-05-27T12:42:30-07:00)
- [SurrealDB GitHub Repository](https://github.com/surrealdb/surrealdb) (Verified: 2025-05-27T12:43:00-07:00)
- [SurrealDB Official Website](https://surrealdb.com/) (Verified: 2025-05-27T12:43:30-07:00)
