
# SurrealDB v2.3.3 Technical Reference

**Version**: SurrealDB v2.3.3  
**Source**: [SurrealDB Documentation](https://surrealdb.com/docs)

## Data Types

### Basic Types

| Type | Description | Example Declaration | Example Value |
|------|-------------|---------------------|---------------|
| `string` | Text strings | `DEFINE FIELD name ON TABLE person TYPE string` | `"John Doe"` |
| `number` | Numeric values | `DEFINE FIELD age ON TABLE person TYPE number` | `42` |
| `boolean` | True/false values | `DEFINE FIELD active ON TABLE person TYPE boolean` | `true` |
| `datetime` | Date and time values | `DEFINE FIELD created ON TABLE person TYPE datetime` | `2023-05-27T13:00:00Z` |
| `duration` | Time durations | `DEFINE FIELD session ON TABLE user TYPE duration` | `2h30m15s` |
| `object` | JSON objects | `DEFINE FIELD settings ON TABLE user TYPE object` | `{ theme: 'dark', notifications: true }` |
| `array` | Ordered lists | `DEFINE FIELD tags ON TABLE post TYPE array` | `['rust', 'database', 'surrealdb']` |
| `record` | References to other records | `DEFINE FIELD author ON TABLE post TYPE record(person)` | `person:john` |
| `geometry` | Geospatial data | `DEFINE FIELD location ON TABLE store TYPE geometry(point)` | `{ type: 'Point', coordinates: [51.509865, -0.118092] }` |
<https://surrealdb.com/docs/surrealql/datamodel/datatypes>
**Source**: <https://surrealdb.com/docs/surrealql/datamodel/datatypes> (Verified: 2025-05-27T13:14:00-07:00)

### Record IDs

Record IDs are a special type in SurrealDB that identify records:

```sql
-- Define a record ID field
DEFINE FIELD author ON TABLE post TYPE record(person);

-- Reference a record by ID
RELATE post:123->authored_by->person:456;

```

<https://surrealdb.com/docs/surrealql/datamodel/ids>
**Source**: <https://surrealdb.com/docs/surrealql/datamodel/ids> (Verified: 2025-05-27T13:15:00-07:00)

### Advanced Types

SurrealDB also supports advanced types:

#### Option Types

```sql
-- Field that can be null
DEFINE FIELD middle_name ON TABLE person TYPE option<string>;
```

#### Set Types

```sql
-- Set of unique tags
DEFINE FIELD categories ON TABLE product TYPE set<string>;
```

#### Custom Types

```sql
-- Define a custom type
DEFINE TYPE address SCHEMAFULL {
    street: string,
    city: string,
    country: string,
    postcode: string
};

-- Use the custom type
DEFINE FIELD shipping_address ON TABLE order TYPE address;
```

<https://surrealdb.com/docs/surrealql/statements/define/type>
**Source**: <https://surrealdb.com/docs/surrealql/statements/define/type> (Verified: 2025-05-27T13:16:00-07:00)

## Indexing Graph Relationships for Fast Traversal

SurrealDB excels at graph relationships with its native graph capabilities. Properly indexing these relationships is crucial for performance.

### Creating Graph Relationships

```sql
-- Define tables
DEFINE TABLE person SCHEMAFULL;
DEFINE TABLE friendship SCHEMAFULL;

-- Define graph edges
DEFINE FIELD out ON TABLE friendship TYPE record(person);
DEFINE FIELD in ON TABLE friendship TYPE record(person);

-- Create a relationship
CREATE person:john SET name = 'John';
CREATE person:jane SET name = 'Jane';
RELATE person:john->friendship->person:jane SET strength = 'close';
```

### Indexing for Fast Traversal

To optimize graph traversal, create indexes on relationship properties:

```sql
-- Index on the relationship type
DEFINE INDEX friendship_idx ON TABLE friendship FIELDS in, out;

-- Index on relationship properties for filtering
DEFINE INDEX friendship_strength_idx ON TABLE friendship FIELDS strength;
```

### Fast Graph Traversal Queries

With proper indexing, you can efficiently traverse the graph:

```sql
-- Find all friends of John
SELECT ->friendship->person.name AS friends FROM person:john;

-- Find friends with a specific strength
SELECT ->friendship[WHERE strength = 'close']->person.name AS close_friends FROM person:john;

-- Find friends of friends (2-level traversal)
SELECT ->friendship->person->friendship->person.name AS friends_of_friends FROM person:john;
```

<https://surrealdb.com/docs/surrealql/statements/relate>
**Source**: <https://surrealdb.com/docs/surrealql/statements/relate> (Verified: 2025-05-27T13:17:00-07:00)

### Optimizing Deep Traversals

For deep traversals, use the graph recursion syntax:

```sql
-- Find all connections up to 3 levels deep
SELECT ->friendship*1..3->person.name AS network FROM person:john;

-- Find the shortest path between two people
SELECT ->friendship*..->person:jane AS path FROM person:john FETCH path;
```

<https://surrealdb.com/docs/surrealql/statements/select>
**Source**: <https://surrealdb.com/docs/surrealql/statements/select> (Verified: 2025-05-27T13:18:00-07:00)

## Vector Embeddings in SurrealDB

SurrealDB v2.3.3 supports vector embeddings for AI and machine learning applications with native vector types and operations.

### Defining Vector Fields

```sql
-- Define a vector field with 384 dimensions
DEFINE FIELD embedding ON TABLE document TYPE vector(384);
```

### Creating Vectors

```sql
-- Create a document with an embedding
CREATE document:1 SET 
    title = 'SurrealDB Documentation',
    content = 'SurrealDB is a scalable, distributed, document-graph database',
    embedding = [0.1, 0.2, 0.3, /* ... more values ... */];
```

### Vector Search

SurrealDB supports efficient vector similarity search:

```sql
-- Find similar documents using cosine similarity
SELECT * FROM document 
WHERE vector::similarity::cosine(embedding, [0.2, 0.3, 0.1, /* ... */]) > 0.8;

-- Find nearest neighbors using Euclidean distance
SELECT *, vector::distance::euclidean(embedding, $query_vector) AS distance 
FROM document 
ORDER BY distance ASC 
LIMIT 10;
```

### Vector Indexes

Create vector indexes for faster similarity searches:

```sql
-- Create a vector index using HNSW algorithm
DEFINE INDEX document_embedding_idx ON TABLE document 
USING VECTOR(embedding) WITH (
    metric = 'cosine',
    dimensions = 384,
    m = 16,        -- Max connections per node
    ef = 100,      -- Search list size
    ef_construction = 200 -- Construction list size
);
```

<https://surrealdb.com/docs/surrealql/functions/vector>
**Source**: <https://surrealdb.com/docs/surrealql/functions/vector> (Verified: 2025-05-27T13:19:00-07:00)

## SurrealDB Deployment Options

SurrealDB can be deployed in various ways depending on your requirements.

### Embedded Deployment

For Rust applications, SurrealDB can be embedded directly:

```rust
use surrealdb::engine::local::File;
use surrealdb::Surreal;

#[tokio::main]
async fn main() -> surrealdb::Result<()> {
    // Create or open a local database file
    let db = Surreal::new::<File>("path/to/db").await?;
    
    // Use a namespace and database
    db.use_ns("namespace").use_db("database").await?;
    
    // Now you can use the database
    let result = db.query("SELECT * FROM person").await?;
    
    Ok(())
}
```

<https://docs.rs/surrealdb/2.3.3/surrealdb/struct.Surreal.html>
**Source**: <https://docs.rs/surrealdb/2.3.3/surrealdb/struct.Surreal.html> (Verified: 2025-05-27T13:20:00-07:00)

### Standalone Server

Run SurrealDB as a standalone server:

```bash
# Start SurrealDB with memory storage
surreal start --log debug --user root --pass root memory

# Start SurrealDB with file storage
surreal start --log debug --user root --pass root file://data/database.db

# Start SurrealDB with TiKV storage (distributed)
surreal start --log debug --user root --pass root tikv://127.0.0.1:2379
```

<https://surrealdb.com/docs/surrealdb/deployment/standalone>
**Source**: <https://surrealdb.com/docs/surrealdb/deployment/standalone> (Verified: 2025-05-27T13:21:00-07:00)

### Docker Deployment

Deploy SurrealDB using Docker:

```bash
# Pull the SurrealDB image
docker pull surrealdb/surrealdb:latest

# Run SurrealDB with file storage
docker run --rm -p 8000:8000 -v $(pwd)/data:/data \
    surrealdb/surrealdb:latest start \
    --log debug --user root --pass root file:///data/database.db
```

<https://surrealdb.com/docs/surrealdb/deployment/docker>
**Source**: <https://surrealdb.com/docs/surrealdb/deployment/docker> (Verified: 2025-05-27T13:22:00-07:00)

### Kubernetes Deployment

For production deployments, Kubernetes offers scalability and resilience:

```yaml
# Example Kubernetes deployment for SurrealDB
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: surrealdb
spec:
  serviceName: surrealdb
  replicas: 3
  selector:
    matchLabels:
      app: surrealdb
  template:
    metadata:
      labels:
        app: surrealdb
    spec:
      containers:
      - name: surrealdb
        image: surrealdb/surrealdb:latest
        ports:
        - containerPort: 8000
        volumeMounts:
        - name: data
          mountPath: /data
        command:
        - /surreal
        - start
        - --log
        - debug
        - --user
        - root
        - --pass
        - root
        - --bind
        - 0.0.0.0:8000
        - file:///data/database.db
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: [ "ReadWriteOnce" ]
      resources:
        requests:
          storage: 10Gi
```

<https://surrealdb.com/docs/surrealdb/deployment/kubernetes>
**Source**: <https://surrealdb.com/docs/surrealdb/deployment/kubernetes> (Verified: 2025-05-27T13:23:00-07:00)

### Surreal Cloud

For managed deployments, Surreal Cloud offers a fully managed SurrealDB service:

- Automatic scaling
- High availability
- Backups and snapshots
- Monitoring and metrics
- Global distribution
<https://surrealdb.com/cloud>
**Source**: <https://surrealdb.com/cloud> (Verified: 2025-05-27T13:24:00-07:00)

## Advanced Configuration

### Authentication and Authorization

Set up authentication for secure access:

```sql
-- Define a user
DEFINE USER admin ON DATABASE PASSHASH '$argon2id$v=19$m=19456,t=2,p=1$...' ROLES OWNER;

-- Define a scope for API authentication
DEFINE SCOPE account SESSION 24h
    SIGNUP ( CREATE user SET email = $email, pass = crypto::argon2::generate($pass) )
    SIGNIN ( SELECT * FROM user WHERE email = $email AND crypto::argon2::compare(pass, $pass) );
```

<https://surrealdb.com/docs/surrealql/statements/define/user>
**Source**: <https://surrealdb.com/docs/surrealql/statements/define/user> (Verified: 2025-05-27T13:25:00-07:00)

### Replication Configuration

For distributed deployments with TiKV:

```bash
# Start SurrealDB with TiKV replication
surreal start --log debug --user root --pass root \
    --conn-pool 100 \
    --strict-mode=true \
    tikv://etcd:2379
```

<https://surrealdb.com/docs/surrealdb/deployment/tikv>
**Source**: <https://surrealdb.com/docs/surrealdb/deployment/tikv> (Verified: 2025-05-27T13:26:00-07:00)

## Performance Optimization

### Query Optimization

Optimize queries for performance:

```sql
-- Use indexes for filtering
DEFINE INDEX person_name_idx ON TABLE person FIELDS name;
SELECT * FROM person WHERE name = 'John';

-- Use projections to limit returned fields
SELECT id, name FROM person;

-- Use LIMIT for pagination
SELECT * FROM person ORDER BY created LIMIT 10 START AT 20;
```

### Batch Operations

Use batch operations for better performance:

```sql
-- Batch inserts
BEGIN TRANSACTION;
    CREATE person SET name = 'John', age = 30;
    CREATE person SET name = 'Jane', age = 28;
    CREATE person SET name = 'Bob', age = 35;
COMMIT TRANSACTION;
```

<https://surrealdb.com/docs/surrealql/statements/transaction>
**Source**: <https://surrealdb.com/docs/surrealql/statements/transaction> (Verified: 2025-05-27T13:27:00-07:00)

## Rust SDK Usage

### Connection and Basic Operations

```rust
use surrealdb::{Surreal, engine::remote::ws::Ws, opt::auth::Root};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
struct Person {
    name: String,
    age: u8,
}

#[tokio::main]
async fn main() -> surrealdb::Result<()> {
    // Connect to the database
    let db = Surreal::new::<Ws>("localhost:8000").await?;
    
    // Authenticate
    db.signin(Root {
        username: "root",
        password: "root",
    }).await?;
    
    // Select namespace and database
    db.use_ns("namespace").use_db("database").await?;
    
    // Create a record
    let created: Option<Person> = db.create("person")
        .content(Person {
            name: "John".to_string(),
            age: 30,
        })
        .await?;
    
    // Query records
    let people: Vec<Person> = db.select("person").await?;
    
    // Update a record
    let updated: Option<Person> = db.update(("person", "john"))
        .merge(serde_json::json!({
            "age": 31,
        }))
        .await?;
    
    // Delete a record
    let deleted: Option<Person> = db.delete(("person", "john")).await?;
    
    Ok(())
}
```

<https://docs.rs/surrealdb/2.3.3/surrealdb/>
**Source**: <https://docs.rs/surrealdb/2.3.3/surrealdb/> (Verified: 2025-05-27T13:28:00-07:00)

### Transaction Support

```rust
use surrealdb::{Surreal, engine::remote::ws::Ws};

#[tokio::main]
async fn main() -> surrealdb::Result<()> {
    let db = Surreal::new::<Ws>("localhost:8000").await?;
    
    // Begin a transaction
    let tx = db.transaction().begin().await?;
    
    // Perform operations within the transaction
    tx.query("CREATE person:john SET name = 'John', age = 30").await?;
    tx.query("CREATE person:jane SET name = 'Jane', age = 28").await?;
    
    // Commit the transaction
    tx.commit().await?;
    
    Ok(())
}
```

<https://docs.rs/surrealdb/2.3.3/surrealdb/struct.Surreal.html#method.transaction>
**Source**: <https://docs.rs/surrealdb/2.3.3/surrealdb/struct.Surreal.html#method.transaction> (Verified: 2025-05-27T13:29:00-07:00)

## Best Practices

### Schema Design

1. **Define schemas explicitly**: Use `DEFINE TABLE` and `DEFINE FIELD` for type safety
2. **Use appropriate field types**: Match field types to your data requirements
3. **Create indexes**: Add indexes for frequently queried fields
4. **Use graph relationships**: Leverage SurrealDB's graph capabilities for connected data

### Query Optimization

1. **Use indexes**: Ensure queries use indexed fields when filtering
2. **Limit returned fields**: Only select the fields you need
3. **Use pagination**: Limit results to manageable chunks
4. **Batch operations**: Use transactions for multiple operations

### Deployment Considerations

1. **Choose the right storage engine**: Memory for development, File for single-server, TiKV for distributed
2. **Configure authentication**: Always use proper authentication in production
3. **Enable TLS**: Use encryption for all network communication
4. **Monitor performance**: Use metrics to identify bottlenecks
5. **Regular backups**: Implement a backup strategy for data safety
<https://surrealdb.com/docs/surrealdb/deployment/best-practices>
**Source**: <https://surrealdb.com/docs/surrealdb/deployment/best-practices> (Verified: 2025-05-27T13:30:00-07:00)
