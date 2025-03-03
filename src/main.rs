use clap::Parser;
use pubky_nexus::{_service::NexusApi, _watcher::NexusWatcher, types::DynError};
use pubky_nexus::cli::{Cli, DbCommands, MigrationCommands, MockType, NexusCommands};
use tokio::join;

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let cli = Cli::parse();

    println!("{:?}", cli);

    match cli.command.unwrap_or(NexusCommands::All) {
        NexusCommands::Api => {
            println!("Starting API...");
            // Run service logic here
            NexusApi::builder().run().await?
        }
        NexusCommands::Watcher => {
            println!("Starting watcher...");
            // Run watcher logic here
            NexusWatcher::builder().run().await?
        }
        NexusCommands::Db(db_command) => {
            match db_command {
                DbCommands::Clear => {
                    println!("Clearing database...");
                }
                DbCommands::Mock(args) => {
                    match args.mock_type {
                        Some(MockType::Redis) => println!("Mocking Redis database..."),
                        Some(MockType::Graph) => println!("Mocking Graph database..."),
                        None => println!("Mocking both Redis and Graph databases..."),
                    }
                }
                DbCommands::Migration(migration_command) => {
                    match migration_command {
                        MigrationCommands::New(args) => {
                            println!("Creating new migration: {}", args.name);
                        }
                        MigrationCommands::Run => {
                            println!("Running pending migrations...");
                        }
                    }
                }
            }
        }
        NexusCommands::All => {
            let (api_result, watcher_result) = join!(
                NexusApi::builder().run(),
                NexusWatcher::builder().run()
            );
        
            // Handle possible errors
            let _ = api_result;
            watcher_result?;
        }
    }    

    Ok(())
}