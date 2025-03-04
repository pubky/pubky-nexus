use clap::Parser;
use pubky_nexus::cli::{Cli, DbCommands, MigrationCommands, NexusCommands};
use pubky_nexus::mock_db::MockDb;
use pubky_nexus::{_service::NexusApi, _watcher::NexusWatcher, types::DynError};
use tokio::join;

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(NexusCommands::All) {
        NexusCommands::Api(args) => {
            if let Some(config_file) = args.config {
                NexusApi::run_with_config_file(config_file).await?
            } else {
                println!("Starting api service...");
                // Run watcher logic here
                NexusApi::builder().run().await?
            }
        }
        NexusCommands::Watcher(args) => {
            if let Some(config_file) = args.config {
                NexusWatcher::run_with_config_file(config_file).await?
            } else {
                println!("Starting watcher...");
                // Run watcher logic here
                NexusWatcher::builder().run().await?
            }
        }
        NexusCommands::Db(db_command) => match db_command {
            DbCommands::Clear => MockDb::clear_database().await,
            DbCommands::Mock(args) => MockDb::run(args.mock_type).await,
            DbCommands::Migration(migration_command) => match migration_command {
                MigrationCommands::New(args) => {
                    println!("Creating new migration: {}", args.name);
                }
                MigrationCommands::Run => {
                    println!("Running pending migrations...");
                }
            },
        },
        NexusCommands::All => {
            let (api_result, watcher_result) =
                join!(NexusApi::builder().run(), NexusWatcher::builder().run());

            // Handle possible errors
            let _ = api_result;
            watcher_result?;
        }
    }

    Ok(())
}
