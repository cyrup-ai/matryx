# SurrealDB v2.3.3 Technical Reference

**Version**: 2.3.3  
**Last Updated**: 2025-05-27T13:21:27-07:00  
**Source**: [SurrealDB Documentation](https://surrealdb.com/docs) [Last Verified: 2025-05-27T13:21:27-07:00]

## Data Types

### Primitive Types
- `string` - Text values: `"hello"` 
- `number` - Numeric values: `42`, `3.14`
- `boolean` - `true` or `false`
- `datetime` - ISO timestamps: `2023-05-27T13:00:00Z`
- `duration` - Time periods: `2h30m15s`
- `null` - Absence of value

**Source**: [SurrealDB Data Types](https://surrealdb.com/docs/surrealql/datamodel/datatypes) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

### Collection Types
- `object` - JSON objects: `{ key: "value" }`
- `array` - Ordered lists: `[1, 2, 3]`
- `set<T>` - Unique collections: `set<string>`

**Source**: [SurrealDB Collection Types](https://surrealdb.com/docs/surrealql/datamodel/arrays) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

### Reference Types
- `record(table)` - Links to other records: `person:john`
- `record<table>` - Parametric record type
- `option<T>` - Nullable value: `option<string>`

**Source**: [SurrealDB Record IDs](https://surrealdb.com/docs/surrealql/datamodel/ids) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

### Specialized Types
- `vector(N)` - Vector embeddings with N dimensions
- `geometry` - Geospatial data: points, polygons

**Source**: [SurrealDB Vector Functions](https://surrealdb.com/docs/surrealql/functions/vector) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

## TYPE SELECTION GUIDE

| Use Type         | When To Use                           | Avoid When                          |
|------------------|---------------------------------------|-------------------------------------|
| `string`         | Text, identifiers, readable values    | Large text (use object storage)     |
| `number`         | Quantities, measurements, scores      | Precise financial (use string)      |
| `datetime`       | Timestamps, calendar events           | Time ranges (use duration)          |
| `record(table)`  | Direct relationships                  | Many-to-many (use edge tables)      |
| `vector`         | ML embeddings, similarity search      | Simple text search (use index)      |
| `option<T>`      | Values that might be null             | Required fields                     |

**Source**: [SurrealDB Schema Design](https://surrealdb.com/docs/surrealql/statements/define) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

## Graph Relationships

### Creating Graph Relationships

```sql
-- Define tables
DEFINE TABLE person SCHEMAFULL;
DEFINE TABLE friendship SCHEMAFULL;

-- Define graph edges
DEFINE FIELD out ON TABLE friendship TYPE record(person);
DEFINE FIELD in ON TABLE friendship TYPE record(person);

-- Create relationship
RELATE person:john->friendship->person:jane SET strength = 'close';
```

**Source**: [SurrealDB Graph Relationships](https://surrealdb.com/docs/surrealql/statements/relate) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

### Fast Graph Traversal Indexes

```sql
-- Index relationships for fast traversal
DEFINE INDEX friendship_idx ON TABLE friendship FIELDS in, out;

-- Access pattern index (critical for performance)
DEFINE INDEX friendship_strength_idx ON TABLE friendship FIELDS strength;
```

**Source**: [SurrealDB Indexing](https://surrealdb.com/docs/surrealql/statements/define/index) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

## GRAPH OPTIMIZATION GUIDE

| Pattern                     | When Effective                           | Performance Impact               |
|-----------------------------|------------------------------------------|----------------------------------|
| Direct edge indexes         | High-frequency traversal paths           | 10-100x speedup on large graphs  |
| Multi-property indexes      | Filtered edge traversals                 | Critical for WHERE clauses       |
| Avoid deep recursion        | Use `*1..3` instead of unbounded `*`     | Prevents exponential query time  |
| Denormalize common paths    | Pre-compute frequently accessed paths    | For read-heavy applications      |

**Source**: [SurrealDB Query Optimization](https://surrealdb.com/docs/surrealql/statements/select) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

## Vector Embeddings

### Creating Vector Fields

```sql
-- Define vector field with 384 dimensions
DEFINE FIELD embedding ON TABLE document TYPE vector(384);

-- Create vector index for similarity search
DEFINE INDEX document_embedding_idx ON TABLE document 
USING VECTOR(embedding) WITH (
    metric = 'cosine',
    dimensions = 384,
    m = 16, 
    ef = 100
);
```

**Source**: [SurrealDB Vector Types](https://surrealdb.com/docs/surrealql/datamodel/vectors) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

### Vector Search Operations

```sql
-- Find similar documents (cosine similarity)
SELECT *,
    vector::similarity::cosine(embedding, $query_vector) AS score
FROM document 
WHERE vector::similarity::cosine(embedding, $query_vector) > 0.8
ORDER BY score DESC
LIMIT 10;
```

**Source**: [SurrealDB Vector Functions](https://surrealdb.com/docs/surrealql/functions/vector) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

## VECTOR SEARCH GUIDE

| Metric    | Best For                       | Trade-offs                     |
|-----------|--------------------------------|--------------------------------|
| `cosine`  | Text embeddings, semantic search | Less sensitive to magnitude  |
| `euclid`  | Image embeddings, clustering     | More sensitive to scaling    |
| `dot`     | Trained embeddings with normalization | Fastest computation     |

| Parameter        | Performance Impact                    | Memory Usage          |
|------------------|--------------------------------------|------------------------|
| `m` (connections) | Higher = better recall, slower writes | Linear increase      |
| `ef` (search size)| Higher = better recall, slower queries | No memory impact    |
| `ef_construction` | Higher = better index quality, slower builds | Build-time only|

**Source**: [SurrealDB Vector Indexing](https://surrealdb.com/docs/surrealql/statements/define/index) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

## Deployment Options

### Embedded (Rust)

```rust
use surrealdb::engine::local::File;
use surrealdb::Surreal;

// Create database connection
let db = Surreal::new::<File>("path/to/db").await?;
    
// Use namespace and database
db.use_ns("namespace").use_db("database").await?;
```

**Source**: [SurrealDB Rust API](https://docs.rs/surrealdb/2.3.3/surrealdb/struct.Surreal.html) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

### Standalone Server

```bash
# Memory storage (development)
surreal start --log debug --user root --pass root memory

# File storage (single server)
surreal start --log debug --user root --pass root file://data/database.db

# TiKV storage (distributed)
surreal start --log debug --user root --pass root tikv://127.0.0.1:2379
```

**Source**: [SurrealDB Deployment](https://surrealdb.com/docs/deployment/overview) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

## DEPLOYMENT SELECTION GUIDE

| Storage Engine | Best For                 | Limitations                     | Data Durability            |
|----------------|--------------------------|--------------------------------|----------------------------|
| `memory`       | Development, testing     | Data lost on restart           | None                       |
| `file`         | Single-server production | Limited to one instance        | Consistent disk writes     |
| `tikv`         | Distributed production   | Requires etcd coordination     | Replicated across nodes    |
| `surreal cloud`| Managed production       | Monthly cost                   | Automated backup system    |

**Source**: [SurrealDB Storage Options](https://surrealdb.com/docs/surrealdb/deployment/overview) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]

## Basic Operations in Rust

```rust
// Connect to database
let db = Surreal::new::<Ws>("localhost:8000").await?;

// Authenticate
db.signin(Root {
    username: "root",
    password: "root",
}).await?;

// Create record
let created: Option<Person> = db.create("person")
    .content(Person {
        name: "John".to_string(),
        age: 30,
    })
    .await?;

// Select records
let people: Vec<Person> = db.select("person").await?;

// Transaction
let tx = db.transaction().begin().await?;
tx.query("CREATE person:john SET name = 'John'").await?;
tx.commit().await?;
```

**Source**: [SurrealDB Rust API Reference](https://docs.rs/surrealdb/2.3.3/surrealdb/) [Last Accessed: 2025-05-27T13:21:27-07:00] [Last Verified: 2025-05-27T13:21:27-07:00]
