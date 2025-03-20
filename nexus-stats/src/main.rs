use clap::Parser;
use nexus_common::{db::Neo4JConfig, StackManager};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

mod stats_service;

// Define CLI arguments.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    neo4_password: String,
    neo4_url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up the tracing subscriber to print debug-level logs to the terminal.
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    tracing::debug!("Starting application with debug tracing enabled");

    // Parse CLI arguments.
    let args = Args::parse();
    tracing::debug!("Parsed CLI arguments: {:?}", args);

    // Load the configuration for Neo4j.
    let neo4j_config = Neo4JConfig {
        uri: args.neo4_url,
        user: "neo4j".to_string(),
        password: args.neo4_password,
    };

    tracing::debug!("Setting up Neo4j connection...");
    StackManager::setup_neo4j(&neo4j_config).await;
    tracing::debug!("Neo4j connection established.");

    // Run the observability metrics service.
    stats_service::run_metrics().await?;
    Ok(())
}
