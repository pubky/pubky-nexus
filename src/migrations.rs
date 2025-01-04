use std::env;

use pubky_nexus::types::DynError;
use pubky_nexus::{get_migration_manager, get_neo4j_graph, setup, Config, MigrationManager};

/// Migration manager entry point
#[tokio::main]
async fn main() -> Result<(), DynError> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("new") => {
            println!("Creating a new migration file...");
            let name = match args.get(2).map(String::as_str) {
                Some(name) => name,
                None => {
                    eprintln!("Usage: cargo run --bin migrations new <name>");
                    return Err("No migration name provided".into());
                }
            };
            MigrationManager::new_migration(name).await?;
            Ok(())
        }
        Some("run") => {
            let config = Config::from_env();
            setup(&config).await;
            log::info!("Running all pending migrations...");
            let graph = get_neo4j_graph()?;
            let migration_manager = get_migration_manager(graph);
            migration_manager.run(&config).await?;
            Ok(())
        }
        _ => {
            log::error!("Usage: cargo run --bin migrations [new|run]");
            Err("Invalid command".into())
        }
    }
}
