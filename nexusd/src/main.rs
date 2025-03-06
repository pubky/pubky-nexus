use clap::Parser;
use nexus_api::{builder::NexusApi, mock::MockDb};
use nexus_watcher::builder::NexusWatcher;
use nexusd::cli::{Cli, DbCommands, MigrationCommands, NexusCommands};
use nexusd::migrations::{import_migrations, MigrationBuilder, MigrationManager};
use std::error::Error;
use tokio::join;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
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
                MigrationCommands::New(args) => MigrationManager::new_migration(args.name).await?,
                MigrationCommands::Run => {
                    let builder = MigrationBuilder::default().await;
                    let mut mm = builder.init_stack().await;
                    import_migrations(&mut mm);
                    mm.run(&builder.migrations_backfill_ready()).await?;
                }
            }
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
