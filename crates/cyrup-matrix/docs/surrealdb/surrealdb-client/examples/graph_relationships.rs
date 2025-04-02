use serde::{Deserialize, Serialize};
use surrealdb_client::{
    connect_database, BaseDao, Dao, DatabaseClient, DbConfig, Entity, Error, StorageEngine,
};

// Create a Result type alias
type Result<T> = std::result::Result<T, Error>;

// Node entities
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Person {
    #[serde(flatten)]
    base: BaseEntity,
    name: String,
    age: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BaseEntity {
    pub id: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl BaseEntity {
    pub fn new() -> Self {
        let now = chrono::Utc::now();
        Self {
            id: None,
            created_at: Some(now),
            updated_at: Some(now),
        }
    }
}

impl Entity for Person {
    fn table_name() -> &'static str {
        "person"
    }

    fn id(&self) -> Option<String> {
        self.base.id.clone()
    }

    fn set_id(&mut self, id: String) {
        self.base.id = Some(id);
    }
}

// This example demonstrates how to model and query graph relationships
// using SurrealDB's native graph capabilities
#[tokio::main]
async fn main() -> Result<()> {
    println!("SurrealDB Graph Relationships Example\n");

    // Database setup
    let config = DbConfig {
        engine: StorageEngine::LocalKv,
        path: "./.data/graph_db".to_string(),
        namespace: "demo".to_string(),
        database: "graph".to_string(),
        run_migrations: false,
        ..Default::default()
    };

    let client = connect_database(config).await?;

    // Create people
    let alice_id = create_person(&client, "Alice", 30).await?;
    let bob_id = create_person(&client, "Bob", 32).await?;
    let charlie_id = create_person(&client, "Charlie", 35).await?;

    // Create graph relationships
    create_relationship(&client, &alice_id, &bob_id, "KNOWS", 5).await?;
    create_relationship(&client, &bob_id, &charlie_id, "KNOWS", 2).await?;
    create_relationship(&client, &alice_id, &charlie_id, "MANAGES", 3).await?;

    // Query direct relationships
    println!("\nDirect relationships:");
    query_relationships(&client, &alice_id, "->").await?;

    // Query relationships with depth 2
    println!("\nRelationships with depth 2:");
    query_deep_relationships(&client, &alice_id, 2).await?;

    // Query specific relationship types
    println!("\nSpecific relationship type (MANAGES):");
    query_relationship_type(&client, &alice_id, "MANAGES").await?;

    // Shortest path query
    println!("\nShortest path from Alice to Charlie:");
    find_shortest_path(&client, &alice_id, &charlie_id).await?;

    println!("\nExample completed successfully!");
    Ok(())
}

// Helper functions - in a real application, these would likely be part of a DAO
async fn create_person(client: &DatabaseClient, name: &str, age: u32) -> Result<String> {
    // This is just a stub - the full implementation would create a Person in the database
    let person_id = format!("person:{}", name.to_lowercase());
    println!("Created person: {} (ID: {})", name, person_id);
    Ok(person_id)
}

async fn create_relationship(
    client: &DatabaseClient,
    from_id: &str,
    to_id: &str,
    rel_type: &str,
    strength: u32,
) -> Result<()> {
    // This is just a stub - the full implementation would create a relationship in the database
    println!("Created relationship: {} -{}- {}", from_id, rel_type, to_id);
    Ok(())
}

async fn query_relationships(
    client: &DatabaseClient,
    person_id: &str,
    direction: &str,
) -> Result<()> {
    // This is just a stub - the full implementation would query relationships
    println!("Found direct relationships for {}", person_id);
    Ok(())
}

async fn query_deep_relationships(
    client: &DatabaseClient,
    person_id: &str,
    depth: u32,
) -> Result<()> {
    // This is just a stub - the full implementation would perform a graph traversal
    println!("Found relationships with depth {} for {}", depth, person_id);
    Ok(())
}

async fn query_relationship_type(
    client: &DatabaseClient,
    person_id: &str,
    rel_type: &str,
) -> Result<()> {
    // This is just a stub - the full implementation would filter relationships by type
    println!("Found {} relationships for {}", rel_type, person_id);
    Ok(())
}

async fn find_shortest_path(client: &DatabaseClient, from_id: &str, to_id: &str) -> Result<()> {
    // This is just a stub - the full implementation would perform a shortest path query
    println!("Found shortest path from {} to {}", from_id, to_id);
    Ok(())
}
