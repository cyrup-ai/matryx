use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=migrations/tables");
    
    let out_dir = match env::var("OUT_DIR") {
        Ok(dir) => dir,
        Err(e) => panic!("BUILD FAILED: Cannot read OUT_DIR environment variable: {}", e),
    };
    let dest_path = Path::new(&out_dir).join("migrations.surql");
    
    let migrations_dir = Path::new("migrations/tables");
    
    // Read all .surql files in order
    let entries_result = fs::read_dir(migrations_dir);
    let mut entries: Vec<_> = match entries_result {
        Ok(dir) => dir.filter_map(|e| e.ok()).collect(),
        Err(e) => panic!("BUILD FAILED: Cannot read migrations directory 'migrations/tables': {}", e),
    };
    
    // Filter for .surql files only
    entries.retain(|e| {
        e.path()
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s == "surql")
            .unwrap_or(false)
    });
    
    // Sort by filename to maintain migration order (000-096)
    entries.sort_by_key(|e| e.path());
    
    let mut combined = String::new();
    
    for entry in entries {
        let path = entry.path();
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => panic!("BUILD FAILED: Cannot read migration file {:?}: {}", path, e),
        };
        combined.push_str(&content);
        combined.push_str("\n\n");
    }
    
    if let Err(e) = fs::write(&dest_path, combined) {
        panic!("BUILD FAILED: Cannot write combined migrations file to {:?}: {}", dest_path, e);
    }
}
